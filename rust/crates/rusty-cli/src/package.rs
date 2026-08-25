//! Deterministic pre-desktop Product Assembly packaging.
//!
//! This module deliberately stops at a sealed input for a later selected
//! desktop wrapper.  It does not select, build, or imply a Tauri/Electron
//! implementation.  The package lives in the separate, fixed
//! `generated/product-package` lane so it cannot add files to the exact
//! browser-bundle closure owned by Product Assembly.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use product_assembly::{decode_assembly_receipt, AssemblyReceipt};
use product_model::{decode_product_manifest, ProductManifest, WrapperKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Fixed, Engine-owned output for a pre-desktop runtime package.  This is an
/// input to wrapper realization, not a wrapper output or installation.
pub(crate) const PRODUCT_PACKAGE_DIRECTORY: &str = "generated/product-package";

const PACKAGE_ARTIFACT: &str = "rusty.product.package";
const PACKAGE_RECEIPT: &str = "package.json";
const ASSEMBLY_RECEIPT: &str = "assembly.json";
const CLOSURE_PREFIX: &str = "closure";
const RUNTIME_BINARY: &str = "closure/runtime/product-runtime";
const MANIFEST_COPY: &str = "closure/rusty.toml";
const LOCK_NAME: &str = ".rusty-product-package-lock";
const STAGE_PREFIX: &str = ".rusty-product-package-stage-";
const MAX_PACKAGE_FILES: usize = 16_512;
const MAX_PACKAGE_BYTES: usize = 512 * 1024 * 1024;
const MAX_BINARY_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageError {
    code: &'static str,
    path: String,
    detail: String,
}

impl PackageError {
    fn new(code: &'static str, path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            detail: detail.into(),
        }
    }

    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at {}: {}", self.code, self.path, self.detail)
    }
}

impl std::error::Error for PackageError {}

/// Readback evidence for a complete package.  Callers can report this
/// without retaining source-root or machine-specific absolute paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackagedProduct {
    product: String,
    package_directory: String,
    package_sha256: String,
    files: usize,
}

impl PackagedProduct {
    pub(crate) fn product(&self) -> &str {
        &self.product
    }

    pub(crate) fn package_directory(&self) -> &str {
        &self.package_directory
    }

    pub(crate) fn package_sha256(&self) -> &str {
        &self.package_sha256
    }

    pub(crate) const fn files(&self) -> usize {
        self.files
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PackageReceipt {
    artifact: String,
    product: String,
    assembly_sha256: String,
    entries: Vec<PackageEntry>,
    wrapper_policy: Vec<WrapperPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct PackageEntry {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct WrapperPolicy {
    id: String,
    kind: String,
    version: String,
    application_id: String,
    title: String,
    window_width: u32,
    window_height: u32,
    resizable: bool,
    permissions: Vec<String>,
    storage_namespace: String,
    release_channel: String,
    singleton: bool,
}

/// Seals the exact admitted Product Assembly closure plus one already-built
/// release binary below [`PRODUCT_PACKAGE_DIRECTORY`].  Existing output is
/// accepted only when its entire tree byte-for-byte equals this package; all
/// drift fails closed and is left untouched.
///
/// `release_binary` is deliberately copied by bytes, not linked.  The
/// resulting package has no runtime path reach-through to the product root or
/// to the build machine.
pub(crate) fn package_product(
    product_root: &Path,
    manifest: &ProductManifest,
    release_binary: &Path,
    assembly_receipt: &AssemblyReceipt,
) -> Result<PackagedProduct, PackageError> {
    let root = checked_product_root(product_root)?;
    let manifest_bytes = read_relative_file(&root, "rusty.toml", "RUSTY_PACKAGE_MANIFEST_READ")?;
    let decoded_manifest = std::str::from_utf8(&manifest_bytes)
        .map_err(|error| {
            PackageError::new(
                "RUSTY_PACKAGE_MANIFEST_UTF8",
                "rusty.toml",
                error.to_string(),
            )
        })
        .and_then(|text| {
            decode_product_manifest(text).map_err(|error| {
                PackageError::new(
                    "RUSTY_PACKAGE_MANIFEST_DECODE",
                    "rusty.toml",
                    error.to_string(),
                )
            })
        })?;
    if &decoded_manifest != manifest {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_DRIFT",
            "rusty.toml",
            "the supplied validated manifest does not match the product-root manifest",
        ));
    }
    if assembly_receipt.product() != manifest.product_id() {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_PRODUCT_MISMATCH",
            "assembly.json",
            "the assembly receipt product does not match rusty.toml",
        ));
    }
    let assembly_bytes = assembly_receipt.json_bytes().map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_ASSEMBLY_RECEIPT",
            "assembly.json",
            error.to_string(),
        )
    })?;

    let mut files = BTreeMap::new();
    insert_file(&mut files, ASSEMBLY_RECEIPT, assembly_bytes.clone())?;
    for entry in assembly_receipt.entries() {
        let source = read_relative_file(&root, entry.path(), "RUSTY_PACKAGE_ASSEMBLY_READ")?;
        if source.len() != entry.byte_length() || sha256_hex(&source) != entry.sha256() {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_ASSEMBLY_DRIFT",
                entry.path(),
                "the product file no longer matches its admitted assembly receipt",
            ));
        }
        insert_file(
            &mut files,
            &format!("{CLOSURE_PREFIX}/{}", entry.path()),
            source,
        )?;
    }
    if files.get(MANIFEST_COPY) != Some(&manifest_bytes) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_CLOSURE",
            MANIFEST_COPY,
            "the exact Product Assembly receipt must include its admitted rusty.toml source",
        ));
    }
    let binary = read_regular_file(
        release_binary,
        "RUSTY_PACKAGE_BINARY_READ",
        MAX_BINARY_BYTES,
    )?;
    insert_file(&mut files, RUNTIME_BINARY, binary)?;

    let policy = wrapper_policy(manifest);
    let entries = package_entries(&files);
    let receipt = PackageReceipt {
        artifact: PACKAGE_ARTIFACT.to_owned(),
        product: manifest.product_id().to_owned(),
        assembly_sha256: sha256_hex(&assembly_bytes),
        entries,
        wrapper_policy: policy,
    };
    let receipt_bytes = receipt_bytes(&receipt)?;
    insert_file(&mut files, PACKAGE_RECEIPT, receipt_bytes.clone())?;

    let generated = root.join("generated");
    ensure_directory(&generated, "generated", "RUSTY_PACKAGE_GENERATED_DIRECTORY")?;
    let _lock = PackageLock::acquire(&generated)?;
    let destination = root.join(PRODUCT_PACKAGE_DIRECTORY);
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            verify_existing_package(&destination, &files)?;
            return Ok(PackagedProduct {
                product: manifest.product_id().to_owned(),
                package_directory: PRODUCT_PACKAGE_DIRECTORY.to_owned(),
                package_sha256: sha256_hex(&receipt_bytes),
                files: files.len(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_DESTINATION_READ",
                PRODUCT_PACKAGE_DIRECTORY,
                error.to_string(),
            ));
        }
    }

    let stage = create_stage(&generated)?;
    let stage_result =
        write_tree(&stage, &files).and_then(|()| verify_existing_package(&stage, &files));
    if let Err(error) = stage_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    match fs::rename(&stage, &destination) {
        Ok(()) => {}
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            return Err(PackageError::new(
                "RUSTY_PACKAGE_PUBLISH",
                PRODUCT_PACKAGE_DIRECTORY,
                format!("staged package was not installed: {error}"),
            ));
        }
    }
    if let Err(error) = verify_existing_package(&destination, &files) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_READBACK",
            PRODUCT_PACKAGE_DIRECTORY,
            format!("published package failed readback: {error}"),
        ));
    }
    Ok(PackagedProduct {
        product: manifest.product_id().to_owned(),
        package_directory: PRODUCT_PACKAGE_DIRECTORY.to_owned(),
        package_sha256: sha256_hex(&receipt_bytes),
        files: files.len(),
    })
}

fn checked_product_root(product_root: &Path) -> Result<PathBuf, PackageError> {
    let metadata = fs::symlink_metadata(product_root).map_err(|error| {
        PackageError::new("RUSTY_PACKAGE_ROOT_READ", "$product", error.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_ROOT_DIRECTORY",
            "$product",
            "product root must be a regular directory, not a symlink",
        ));
    }
    Ok(product_root.to_path_buf())
}

fn read_relative_file(
    root: &Path,
    relative: &str,
    code: &'static str,
) -> Result<Vec<u8>, PackageError> {
    let mut current = root.to_path_buf();
    let components = relative.split('/').collect::<Vec<_>>();
    for component in &components[..components.len().saturating_sub(1)] {
        current.push(component);
        ensure_directory(&current, relative, code)?;
    }
    let file = components
        .last()
        .ok_or_else(|| PackageError::new(code, relative, "empty relative path"))?;
    current.push(file);
    read_regular_file(&current, code, MAX_PACKAGE_BYTES)
}

fn read_regular_file(
    path: &Path,
    code: &'static str,
    limit: usize,
) -> Result<Vec<u8>, PackageError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PackageError::new(code, path.display().to_string(), error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PackageError::new(
            code,
            path.display().to_string(),
            "expected a regular non-symlink file",
        ));
    }
    let size = usize::try_from(metadata.len()).map_err(|_| {
        PackageError::new(
            code,
            path.display().to_string(),
            "file size does not fit this platform",
        )
    })?;
    if size > limit {
        return Err(PackageError::new(
            code,
            path.display().to_string(),
            format!("file exceeds {limit} byte bound"),
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| PackageError::new(code, path.display().to_string(), error.to_string()))?;
    if bytes.len() != size {
        return Err(PackageError::new(
            code,
            path.display().to_string(),
            "file changed while being packaged",
        ));
    }
    Ok(bytes)
}

fn ensure_directory(path: &Path, display: &str, code: &'static str) -> Result<(), PackageError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| PackageError::new(code, display, error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PackageError::new(
            code,
            display,
            "expected a regular non-symlink directory",
        ));
    }
    Ok(())
}

fn insert_file(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    bytes: Vec<u8>,
) -> Result<(), PackageError> {
    if files.len() >= MAX_PACKAGE_FILES {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_FILE_BOUNDS",
            path,
            "package file count exceeds its bound",
        ));
    }
    let total = files
        .values()
        .try_fold(bytes.len(), |total, existing| {
            total.checked_add(existing.len())
        })
        .ok_or_else(|| {
            PackageError::new(
                "RUSTY_PACKAGE_BYTES_BOUNDS",
                path,
                "package byte accounting overflowed",
            )
        })?;
    if total > MAX_PACKAGE_BYTES {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_BYTES_BOUNDS",
            path,
            "package bytes exceed their bound",
        ));
    }
    if files.insert(path.to_owned(), bytes).is_some() {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_PATH_COLLISION",
            path,
            "package paths must be unique",
        ));
    }
    Ok(())
}

fn package_entries(files: &BTreeMap<String, Vec<u8>>) -> Vec<PackageEntry> {
    files
        .iter()
        .map(|(path, bytes)| PackageEntry {
            path: path.clone(),
            bytes: bytes.len(),
            sha256: sha256_hex(bytes),
        })
        .collect()
}

fn receipt_bytes(receipt: &PackageReceipt) -> Result<Vec<u8>, PackageError> {
    let mut bytes = serde_json::to_vec_pretty(receipt).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_RECEIPT_SERIALIZE",
            PACKAGE_RECEIPT,
            error.to_string(),
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn wrapper_policy(manifest: &ProductManifest) -> Vec<WrapperPolicy> {
    let mut policy = manifest
        .wrappers()
        .iter()
        .map(|wrapper| WrapperPolicy {
            id: wrapper.id().to_owned(),
            kind: match wrapper.kind() {
                WrapperKind::Tauri => "tauri".to_owned(),
                WrapperKind::Electron => "electron".to_owned(),
            },
            version: wrapper.version().to_owned(),
            application_id: wrapper.application_id().to_owned(),
            title: wrapper.title().to_owned(),
            window_width: wrapper.window_width(),
            window_height: wrapper.window_height(),
            resizable: wrapper.resizable(),
            permissions: wrapper.permissions().to_vec(),
            storage_namespace: wrapper.storage_namespace().to_owned(),
            release_channel: match wrapper.release_channel() {
                product_model::ReleaseChannel::Development => "development".to_owned(),
                product_model::ReleaseChannel::Preview => "preview".to_owned(),
                product_model::ReleaseChannel::Stable => "stable".to_owned(),
            },
            singleton: wrapper.singleton(),
        })
        .collect::<Vec<_>>();
    policy.sort_by(|left, right| left.id.cmp(&right.id));
    policy
}

/// Verifies a package after it has been copied or relocated without needing
/// its former product root.  This is the handoff boundary consumed by later
/// desktop-wrapper work: the verifier sees bytes and declared policy only.
pub(crate) fn verify_product_package(
    package_directory: &Path,
) -> Result<PackagedProduct, PackageError> {
    ensure_directory(
        package_directory,
        PRODUCT_PACKAGE_DIRECTORY,
        "RUSTY_PACKAGE_READBACK",
    )?;
    let actual = collect_tree(package_directory, package_directory, &mut BTreeMap::new())?;
    let receipt_bytes = actual.get(PACKAGE_RECEIPT).ok_or_else(|| {
        PackageError::new(
            "RUSTY_PACKAGE_RECEIPT_MISSING",
            PACKAGE_RECEIPT,
            "package receipt is required for relocation-safe verification",
        )
    })?;
    let receipt: PackageReceipt = serde_json::from_slice(receipt_bytes).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_RECEIPT_DECODE",
            PACKAGE_RECEIPT,
            error.to_string(),
        )
    })?;
    if receipt.artifact != PACKAGE_ARTIFACT {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_RECEIPT_ARTIFACT",
            PACKAGE_RECEIPT,
            "unsupported package receipt artifact",
        ));
    }
    let manifest_bytes = actual.get(MANIFEST_COPY).ok_or_else(|| {
        PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_MISSING",
            MANIFEST_COPY,
            "package closure lacks rusty.toml",
        )
    })?;
    let manifest_text = std::str::from_utf8(manifest_bytes).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_UTF8",
            MANIFEST_COPY,
            error.to_string(),
        )
    })?;
    let manifest = decode_product_manifest(manifest_text).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_DECODE",
            MANIFEST_COPY,
            error.to_string(),
        )
    })?;
    if manifest.product_id() != receipt.product {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_PRODUCT_MISMATCH",
            PACKAGE_RECEIPT,
            "package receipt product does not match the packaged manifest",
        ));
    }
    if receipt.wrapper_policy != wrapper_policy(&manifest) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_POLICY_DRIFT",
            PACKAGE_RECEIPT,
            "wrapper policy receipt does not match the packaged manifest",
        ));
    }
    let assembly_bytes = actual.get(ASSEMBLY_RECEIPT).ok_or_else(|| {
        PackageError::new(
            "RUSTY_PACKAGE_ASSEMBLY_MISSING",
            ASSEMBLY_RECEIPT,
            "package closure lacks assembly receipt",
        )
    })?;
    if sha256_hex(assembly_bytes) != receipt.assembly_sha256 {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_ASSEMBLY_DRIFT",
            ASSEMBLY_RECEIPT,
            "assembly receipt hash does not match the package receipt",
        ));
    }
    let assembly = decode_assembly_receipt(assembly_bytes).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_ASSEMBLY_RECEIPT",
            ASSEMBLY_RECEIPT,
            error.to_string(),
        )
    })?;
    if assembly.product() != manifest.product_id() {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_PRODUCT_MISMATCH",
            ASSEMBLY_RECEIPT,
            "assembly receipt product does not match the packaged manifest",
        ));
    }
    let mut required_closure =
        BTreeSet::from([ASSEMBLY_RECEIPT.to_owned(), RUNTIME_BINARY.to_owned()]);
    for entry in assembly.entries() {
        if !required_closure.insert(format!("{CLOSURE_PREFIX}/{}", entry.path())) {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CLOSURE_COLLISION",
                entry.path(),
                "assembly entry collides with a reserved package closure path",
            ));
        }
    }
    if !required_closure.contains(MANIFEST_COPY) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_MANIFEST_CLOSURE",
            MANIFEST_COPY,
            "assembly receipt does not retain its admitted rusty.toml source",
        ));
    }
    let expected_paths = receipt
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    if expected_paths.windows(2).any(|pair| pair[0] >= pair[1])
        || expected_paths.contains(&PACKAGE_RECEIPT)
    {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_RECEIPT_ORDER",
            PACKAGE_RECEIPT,
            "package receipt entries must be strictly ordered and self-excluded",
        ));
    }
    let actual_paths = actual
        .keys()
        .filter(|path| path.as_str() != PACKAGE_RECEIPT)
        .map(String::as_str)
        .collect::<Vec<_>>();
    if actual_paths != expected_paths {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_CLOSURE_SHAPE",
            PRODUCT_PACKAGE_DIRECTORY,
            "package tree does not exactly match its receipt closure",
        ));
    }
    if expected_paths.iter().copied().collect::<BTreeSet<_>>()
        != required_closure.iter().map(String::as_str).collect()
    {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_CLOSURE_SHAPE",
            PACKAGE_RECEIPT,
            "package receipt must describe exactly the assembly closure, manifest, and runtime binary",
        ));
    }
    for entry in &receipt.entries {
        let bytes = actual
            .get(&entry.path)
            .expect("checked exact package paths");
        if bytes.len() != entry.bytes || sha256_hex(bytes) != entry.sha256 {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CLOSURE_DRIFT",
                &entry.path,
                "package file does not match its receipt hash",
            ));
        }
    }
    if !actual.contains_key(RUNTIME_BINARY) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_BINARY_MISSING",
            RUNTIME_BINARY,
            "package closure lacks the release runtime binary",
        ));
    }
    Ok(PackagedProduct {
        product: manifest.product_id().to_owned(),
        package_directory: PRODUCT_PACKAGE_DIRECTORY.to_owned(),
        package_sha256: sha256_hex(receipt_bytes),
        files: actual.len(),
    })
}

fn verify_existing_package(
    destination: &Path,
    expected: &BTreeMap<String, Vec<u8>>,
) -> Result<(), PackageError> {
    ensure_directory(
        destination,
        PRODUCT_PACKAGE_DIRECTORY,
        "RUSTY_PACKAGE_CONFLICT",
    )?;
    let actual = collect_tree(destination, destination, &mut BTreeMap::new())?;
    if actual.len() != expected.len() || actual.keys().ne(expected.keys()) {
        return Err(PackageError::new(
            "RUSTY_PACKAGE_CONFLICT",
            PRODUCT_PACKAGE_DIRECTORY,
            "existing package has a different closure; no files were changed",
        ));
    }
    for (path, bytes) in expected {
        if actual.get(path) != Some(bytes) {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                path,
                "existing package bytes differ; no files were changed",
            ));
        }
    }
    verify_product_package(destination).map(|_| ())
}

fn collect_tree(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let entries = fs::read_dir(directory).map_err(|error| {
        PackageError::new(
            "RUSTY_PACKAGE_CONFLICT",
            directory.display().to_string(),
            error.to_string(),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                PRODUCT_PACKAGE_DIRECTORY,
                error.to_string(),
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                path.display().to_string(),
                error.to_string(),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                path.display().to_string(),
                "package must not contain symlinks",
            ));
        }
        if metadata.is_dir() {
            collect_tree(root, &path, files)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                path.display().to_string(),
                "package contains a non-regular file",
            ));
        }
        let relative = path.strip_prefix(root).map_err(|_| {
            PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                path.display().to_string(),
                "package path escaped its root",
            )
        })?;
        let key = relative.to_string_lossy().replace('\\', "/");
        let bytes = read_regular_file(&path, "RUSTY_PACKAGE_CONFLICT", MAX_PACKAGE_BYTES)?;
        if files.insert(key.clone(), bytes).is_some() || files.len() > MAX_PACKAGE_FILES {
            return Err(PackageError::new(
                "RUSTY_PACKAGE_CONFLICT",
                key,
                "package contains duplicate or excessive files",
            ));
        }
    }
    Ok(files.clone())
}

fn create_stage(generated: &Path) -> Result<PathBuf, PackageError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| PackageError::new("RUSTY_PACKAGE_STAGE", "generated", error.to_string()))?
        .as_nanos();
    for attempt in 0..16u32 {
        let stage = generated.join(format!("{STAGE_PREFIX}{nonce}-{attempt}"));
        match fs::create_dir(&stage) {
            Ok(()) => return Ok(stage),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(PackageError::new(
                    "RUSTY_PACKAGE_STAGE",
                    "generated",
                    error.to_string(),
                ))
            }
        }
    }
    Err(PackageError::new(
        "RUSTY_PACKAGE_STAGE",
        "generated",
        "could not allocate an exclusive stage",
    ))
}

fn write_tree(stage: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), PackageError> {
    for (relative, bytes) in files {
        let path = stage.join(relative);
        let parent = path.parent().ok_or_else(|| {
            PackageError::new(
                "RUSTY_PACKAGE_STAGE_WRITE",
                relative,
                "package file has no parent",
            )
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            PackageError::new("RUSTY_PACKAGE_STAGE_WRITE", relative, error.to_string())
        })?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&path).map_err(|error| {
            PackageError::new("RUSTY_PACKAGE_STAGE_WRITE", relative, error.to_string())
        })?;
        use std::io::Write;
        file.write_all(bytes).map_err(|error| {
            PackageError::new("RUSTY_PACKAGE_STAGE_WRITE", relative, error.to_string())
        })?;
        file.sync_all().map_err(|error| {
            PackageError::new("RUSTY_PACKAGE_STAGE_WRITE", relative, error.to_string())
        })?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            stage.join(RUNTIME_BINARY),
            fs::Permissions::from_mode(0o755),
        )
        .map_err(|error| {
            PackageError::new(
                "RUSTY_PACKAGE_STAGE_WRITE",
                RUNTIME_BINARY,
                error.to_string(),
            )
        })?;
    }
    Ok(())
}

struct PackageLock {
    path: PathBuf,
}

impl PackageLock {
    fn acquire(generated: &Path) -> Result<Self, PackageError> {
        let path = generated.join(LOCK_NAME);
        match fs::create_dir(&path) {
            Ok(()) => Ok(Self { path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(PackageError::new(
                "RUSTY_PACKAGE_BUSY",
                PRODUCT_PACKAGE_DIRECTORY,
                "another package publication is active; retry after it completes",
            )),
            Err(error) => Err(PackageError::new(
                "RUSTY_PACKAGE_LOCK",
                PRODUCT_PACKAGE_DIRECTORY,
                error.to_string(),
            )),
        }
    }
}

impl Drop for PackageLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use product_assembly::decode_assembly_receipt;

    #[test]
    fn package_is_idempotent_and_rejects_drift() {
        let root = temporary_root("package-idempotent");
        fs::create_dir_all(root.join("rules")).expect("rules");
        fs::create_dir_all(root.join("ui")).expect("ui");
        fs::create_dir_all(root.join("content")).expect("content");
        fs::create_dir_all(root.join("generated")).expect("generated");
        fs::write(root.join("rules/main.ts"), "export {};\n").expect("rules");
        fs::write(root.join("ui/main.ts"), "export {};\n").expect("ui");
        fs::write(root.join("generated/runtime.bin"), b"binary").expect("binary");
        let manifest_text = "[product]\nid = \"rusty.package-test\"\n\n[runtime_composition]\nentrypoints = [\"rules/main.ts\"]\n\n[lifecycle]\nmode = \"demand\"\n\n[ui]\nentry = \"ui/main.ts\"\n\n[content]\nroot = \"content\"\n\n[outputs]\ncompiled_composition = \"generated/compiled-composition.json\"\nadmitted_runtime_content = \"generated/runtime-content\"\nproduct_assembly = \"generated/product-assembly\"\nproduct_bundle = \"generated/product-bundle\"\n\n[[wrappers]]\nid = \"desktop\"\nkind = \"tauri\"\nversion = \"0.1.0\"\napplication_id = \"com.example.package\"\ntitle = \"Package Test\"\nwindow_width = 1280\nwindow_height = 720\nresizable = true\npermissions = [\"window\"]\nstorage_namespace = \"rusty.package-test\"\nrelease_channel = \"development\"\nsingleton = true\n";
        fs::write(root.join("rusty.toml"), manifest_text).expect("manifest");
        let manifest = decode_product_manifest(manifest_text).expect("manifest");
        let source = b"export {};\n";
        let receipt = decode_assembly_receipt(format!("{{\n  \"artifact\": \"rusty.product.assembly\",\n  \"product\": \"rusty.package-test\",\n  \"entries\": [{{\n    \"kind\": \"authored-source\",\n    \"path\": \"rules/main.ts\",\n    \"bytes\": {},\n    \"sha256\": \"{}\"\n  }}, {{\n    \"kind\": \"authored-source\",\n    \"path\": \"rusty.toml\",\n    \"bytes\": {},\n    \"sha256\": \"{}\"\n  }}]\n}}\n", source.len(), sha256_hex(source), manifest_text.len(), sha256_hex(manifest_text.as_bytes())).as_bytes()).expect("receipt");
        let binary = root.join("generated/runtime.bin");
        let first = package_product(&root, &manifest, &binary, &receipt).expect("first package");
        let package_receipt: PackageReceipt = serde_json::from_slice(
            &fs::read(root.join(PRODUCT_PACKAGE_DIRECTORY).join(PACKAGE_RECEIPT))
                .expect("package receipt"),
        )
        .expect("decode package receipt");
        assert_eq!(package_receipt.wrapper_policy.len(), 1);
        assert_eq!(package_receipt.wrapper_policy[0].version, "0.1.0");
        assert!(package_receipt.wrapper_policy[0].singleton);
        let relocated = root.join("relocated-package");
        fs::rename(root.join(PRODUCT_PACKAGE_DIRECTORY), &relocated).expect("relocate package");
        assert_eq!(
            verify_product_package(&relocated).expect("relocated readback"),
            first
        );
        fs::rename(&relocated, root.join(PRODUCT_PACKAGE_DIRECTORY)).expect("restore package");
        let second = package_product(&root, &manifest, &binary, &receipt).expect("repeat package");
        assert_eq!(first, second);
        fs::write(
            root.join(PRODUCT_PACKAGE_DIRECTORY)
                .join("closure/rules/main.ts"),
            b"drift",
        )
        .expect("drift");
        let error =
            package_product(&root, &manifest, &binary, &receipt).expect_err("drift must fail");
        assert_eq!(error.code(), "RUSTY_PACKAGE_CONFLICT");
        let _ = fs::remove_dir_all(root);
    }

    fn temporary_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rusty-cli-{name}-{nonce}"));
        fs::create_dir(&root).expect("root");
        root
    }
}
