//! Selected desktop-wrapper realization for the Product Model workflow.
//!
//! This command-side owner consumes an already verified Product Assembly and
//! Product Package, stages a separate Tauri build workspace, and publishes
//! only a relocatable release closure. The build workspace is never part of
//! the release and no release file reaches through an absolute path.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use product_model::{
    decode_product_manifest, ProductKernelCapabilityDescriptor, ProductManifest,
    WrapperDeclaration, WrapperKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    kernel_probe::run_bounded,
    package::{verify_product_package, PackagedProduct, PRODUCT_PACKAGE_DIRECTORY},
};

const DESKTOP_DIRECTORY: &str = "generated/product-desktop";
const DESKTOP_RECEIPT: &str = "desktop.json";
const DESKTOP_ARTIFACT: &str = "rusty.product.desktop";
const DESKTOP_BINARY: &str = "bin/product-desktop";
const DESKTOP_CARGO_BINARY: &str = "rusty-product-desktop";
const DESKTOP_FRONTEND: &str = "frontend";
const DESKTOP_BASE_PACKAGE: &str = "product-package";
const DESKTOP_CONFIG: &str = "tauri.conf.json";
const DESKTOP_LAUNCHER: &str = "launcher.sh";
const DESKTOP_ENTRY_TEMPLATE: &str = "desktop-entry.template";
const DESKTOP_POLICY: &str = "desktop-policy.json";
const DESKTOP_CAPABILITY: &str = "capabilities/main.json";
const DESKTOP_ACTIVATION_RECEIPT: &str = "activation.json";
const DESKTOP_STAGE_PREFIX: &str = ".rusty-product-desktop-stage-";
const INSTALL_STAGE_PREFIX: &str = ".rusty-product-install-stage-";
const DESKTOP_BUILD_WORKSPACE: &str = "target/product-desktop-workspace";
const DESKTOP_BUILD_TARGET: &str = "target/product-desktop-cache";
const DESKTOP_BUILD_STAGE_PREFIX: &str = ".rusty-product-desktop-build-stage-";
const DESKTOP_BUILD_BACKUP_PREFIX: &str = ".rusty-product-desktop-build-backup-";
const MAX_DESKTOP_FILES: usize = 16_384;
const MAX_DESKTOP_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_DESKTOP_TOTAL_BYTES: usize = 768 * 1024 * 1024;
const MAX_DESKTOP_TEXT_BYTES: usize = 512 * 1024;
const CARGO_BUILD_TIMEOUT: Duration = Duration::from_secs(600);
const BRIDGE_SYNTAX_TIMEOUT: Duration = Duration::from_secs(10);
// Tauri's generated context requires one RGBA PNG even when the product
// manifest has no authored icon. This is a fixed transparent Engine build
// input, never a product asset and never copied into the published release.
const ENGINE_FALLBACK_ICON_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
static STAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Options for one selected desktop wrapper.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DesktopOptions {
    pub(crate) wrapper_id: Option<String>,
    pub(crate) output_directory: Option<PathBuf>,
}

/// A published desktop release. Paths are owned by the caller's selected
/// output or install root; the receipt itself contains only relative paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopPackage {
    root: PathBuf,
    product: String,
    wrapper_id: String,
    wrapper_version: String,
    application_id: String,
    storage_namespace: String,
    package_sha256: String,
    assembly_sha256: String,
    policy_sha256: String,
    release_sha256: String,
    files: usize,
}

#[allow(dead_code)]
impl DesktopPackage {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn product(&self) -> &str {
        &self.product
    }
    pub(crate) fn wrapper_id(&self) -> &str {
        &self.wrapper_id
    }
    pub(crate) fn wrapper_version(&self) -> &str {
        &self.wrapper_version
    }
    pub(crate) fn application_id(&self) -> &str {
        &self.application_id
    }
    pub(crate) fn storage_namespace(&self) -> &str {
        &self.storage_namespace
    }
    pub(crate) fn binary(&self) -> PathBuf {
        self.root.join(DESKTOP_BINARY)
    }
    pub(crate) fn activation_receipt(&self) -> PathBuf {
        self.root.join(DESKTOP_ACTIVATION_RECEIPT)
    }
    pub(crate) fn package_sha256(&self) -> &str {
        &self.package_sha256
    }
    pub(crate) fn assembly_sha256(&self) -> &str {
        &self.assembly_sha256
    }
    pub(crate) fn policy_sha256(&self) -> &str {
        &self.policy_sha256
    }
    pub(crate) fn release_sha256(&self) -> &str {
        &self.release_sha256
    }
    pub(crate) const fn files(&self) -> usize {
        self.files
    }
}

/// User-scoped installation options. Save/data roots are deliberately absent:
/// installing or rotating a release never acquires data ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallOptions {
    pub(crate) launcher_name: String,
    pub(crate) desktop_entry_name: String,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            launcher_name: "rusty-product".to_owned(),
            desktop_entry_name: "rusty-product.desktop".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledDesktop {
    root: PathBuf,
    release: PathBuf,
    launcher: PathBuf,
    desktop_entry: PathBuf,
    data_root: PathBuf,
    application_id: String,
    storage_namespace: String,
}

#[allow(dead_code)]
impl InstalledDesktop {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
    pub(crate) fn release(&self) -> &Path {
        &self.release
    }
    pub(crate) fn launcher(&self) -> &Path {
        &self.launcher
    }
    pub(crate) fn desktop_entry(&self) -> &Path {
        &self.desktop_entry
    }
    pub(crate) fn data_root(&self) -> &Path {
        &self.data_root
    }
    pub(crate) fn application_id(&self) -> &str {
        &self.application_id
    }
    pub(crate) fn storage_namespace(&self) -> &str {
        &self.storage_namespace
    }
    pub(crate) fn binary(&self) -> PathBuf {
        self.release.join(DESKTOP_BINARY)
    }
    pub(crate) fn activation_receipt(&self) -> PathBuf {
        self.data_root.join(DESKTOP_ACTIVATION_RECEIPT)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopError {
    code: &'static str,
    path: String,
    detail: String,
}

impl DesktopError {
    fn new(code: &'static str, path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            detail: bound_text(detail.into(), MAX_DESKTOP_TEXT_BYTES),
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

impl std::fmt::Display for DesktopError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at {}: {}", self.code, self.path, self.detail)
    }
}

impl std::error::Error for DesktopError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedWrapper {
    id: String,
    version: String,
    application_id: String,
    title: String,
    width: u32,
    height: u32,
    resizable: bool,
    permissions: Vec<String>,
    storage_namespace: String,
    release_channel: String,
    singleton: bool,
}

impl SelectedWrapper {
    fn from_declaration(wrapper: &WrapperDeclaration) -> Self {
        Self {
            id: wrapper.id().to_owned(),
            version: wrapper.version().to_owned(),
            application_id: wrapper.application_id().to_owned(),
            title: wrapper.title().to_owned(),
            width: wrapper.window_width(),
            height: wrapper.window_height(),
            resizable: wrapper.resizable(),
            permissions: wrapper.permissions().to_vec(),
            storage_namespace: wrapper.storage_namespace().to_owned(),
            release_channel: release_channel_name(wrapper.release_channel()).to_owned(),
            singleton: wrapper.singleton(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DesktopReceipt {
    artifact: String,
    product: String,
    wrapper_id: String,
    wrapper_version: String,
    application_id: String,
    storage_namespace: String,
    package_sha256: String,
    assembly_sha256: String,
    config_sha256: String,
    policy_sha256: String,
    frontend_sha256: String,
    files: Vec<DesktopEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CanonicalPackageReceipt {
    artifact: String,
    product: String,
    assembly_sha256: String,
    entries: Vec<CanonicalPackageEntry>,
    wrapper_policy: Vec<CanonicalWrapperPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CanonicalPackageEntry {
    path: String,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CanonicalWrapperPolicy {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DesktopPolicy {
    wrapper_id: String,
    wrapper_version: String,
    application_id: String,
    title: String,
    window_width: u32,
    window_height: u32,
    resizable: bool,
    permissions: Vec<String>,
    storage_namespace: String,
    release_channel: String,
    singleton: bool,
    icon_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DesktopEntry {
    path: String,
    bytes: usize,
    sha256: String,
}

struct StagedReleaseInput<'a> {
    manifest: &'a ProductManifest,
    wrapper: &'a SelectedWrapper,
    assembly_root: &'a Path,
    bundle_root: &'a Path,
    package_root: &'a Path,
    stage: &'a Path,
    packaged: &'a PackagedProduct,
    assembly: &'a product_assembly::AssemblyReceipt,
}

struct StagedRelease {
    final_stage: PathBuf,
    expected: BTreeMap<String, Vec<u8>>,
    receipt: DesktopReceipt,
    package_sha256: String,
    assembly_sha256: String,
}

/// Builds and publishes one selected Tauri wrapper. The product root must
/// already have a current Product Assembly and verified Product Package.
pub(crate) fn build_and_publish(
    product_root: &Path,
    options: &DesktopOptions,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<DesktopPackage, DesktopError> {
    let manifest = read_manifest(product_root)?;
    let wrapper = select_tauri_wrapper(&manifest, options.wrapper_id.as_deref())?;
    let assembly_root = product_root.join(manifest.product_assembly_output().as_str());
    let assembly =
        product_assembly::verify_existing_product_assembly_with_kernel_capabilities(
            product_root,
            &manifest,
            kernel_capabilities,
        )
        .map_err(|error| {
            DesktopError::new(
                "RUSTY_DESKTOP_ASSEMBLY",
                assembly_root.display().to_string(),
                format!(
                    "current Product Assembly is not verified: {error}. Remedy: rerun rusty build before desktop packaging."
                ),
            )
        })?;
    let package_root = product_root.join(PRODUCT_PACKAGE_DIRECTORY);
    let packaged = verify_product_package(&package_root).map_err(package_error)?;
    if packaged.product() != manifest.product_id() {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_PRODUCT",
            package_root.display().to_string(),
            "verified Product Package product does not match the selected Product Assembly",
        ));
    }
    let package_identity = read_canonical_package_receipt(&package_root)?;
    let assembly_bytes = assembly.json_bytes().map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_ASSEMBLY_RECEIPT",
            "assembly.json",
            error.to_string(),
        )
    })?;
    validate_canonical_package_identity(
        &package_identity,
        &manifest,
        &wrapper,
        &sha256_hex(&assembly_bytes),
    )?;
    let bundle_root = product_root.join(manifest.product_bundle_output().as_str());
    let output = options
        .output_directory
        .clone()
        .unwrap_or_else(|| product_root.join(DESKTOP_DIRECTORY));
    let output_parent = output.parent().ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_OUTPUT",
            output.display().to_string(),
            "desktop output must have a parent directory",
        )
    })?;
    ensure_directory(
        output_parent,
        "RUSTY_DESKTOP_OUTPUT",
        "desktop output parent",
    )?;

    let stage = create_stage(output_parent, DESKTOP_STAGE_PREFIX)?;
    let result = build_staged_release(StagedReleaseInput {
        manifest: &manifest,
        wrapper: &wrapper,
        assembly_root: &assembly_root,
        bundle_root: &bundle_root,
        package_root: &package_root,
        stage: &stage,
        packaged: &packaged,
        assembly: &assembly,
    });
    let StagedRelease {
        final_stage,
        expected,
        receipt,
        package_sha256,
        assembly_sha256,
    } = match result {
        Ok(value) => value,
        Err(error) => {
            let _ = remove_tree(&stage);
            return Err(error);
        }
    };
    let release_sha256 = sha256_tree(&expected);
    if fs::symlink_metadata(&output).is_ok() {
        let existing = match verify_relocated(&output) {
            Ok(existing) => existing,
            Err(error) => {
                let _ = remove_tree(&stage);
                return Err(error);
            }
        };
        if let Err(error) = ensure_exact_release_tree(&output, &expected) {
            let _ = remove_tree(&stage);
            return Err(error);
        }
        if existing.product != manifest.product_id()
            || existing.wrapper_id != wrapper.id
            || existing.wrapper_version != wrapper.version
            || existing.application_id != wrapper.application_id
            || existing.storage_namespace != wrapper.storage_namespace
            || existing.package_sha256 != package_sha256
            || existing.assembly_sha256 != assembly_sha256
            || existing.policy_sha256 != receipt.policy_sha256
        {
            let _ = remove_tree(&stage);
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_DRIFT",
                output.display().to_string(),
                "existing published desktop release differs from the current verified Product Package, Assembly, or wrapper policy; no files were changed",
            ));
        }
        let _ = remove_tree(&stage);
        return Ok(existing);
    }
    fs::rename(&final_stage, &output).map_err(|error| {
        let _ = remove_tree(&stage);
        DesktopError::new(
            "RUSTY_DESKTOP_PUBLISH",
            output.display().to_string(),
            format!("atomic desktop release publication failed: {error}"),
        )
    })?;
    let _ = remove_tree(&stage);
    Ok(DesktopPackage {
        root: output,
        product: manifest.product_id().to_owned(),
        wrapper_id: wrapper.id,
        wrapper_version: wrapper.version,
        application_id: wrapper.application_id.clone(),
        storage_namespace: wrapper.storage_namespace.clone(),
        package_sha256,
        assembly_sha256,
        policy_sha256: receipt.policy_sha256,
        release_sha256,
        files: expected.len(),
    })
}

fn build_staged_release(input: StagedReleaseInput<'_>) -> Result<StagedRelease, DesktopError> {
    let StagedReleaseInput {
        manifest,
        wrapper,
        assembly_root,
        bundle_root,
        package_root,
        stage,
        packaged,
        assembly,
    } = input;
    let workspace = stage.join("workspace");
    create_directory(&workspace, "RUSTY_DESKTOP_WORKSPACE")?;
    let build_workspace = desktop_build_workspace_path();
    ensure_or_create_directory(&build_workspace, "RUSTY_DESKTOP_BUILD_WORKSPACE")?;
    let staged_assembly = workspace.join("product-assembly");
    let staged_source_bundle = workspace.join("product-bundle");
    let staged_package = workspace.join(DESKTOP_BASE_PACKAGE);
    let staged_frontend = workspace.join(DESKTOP_FRONTEND);
    copy_tree(
        assembly_root,
        &staged_assembly,
        "RUSTY_DESKTOP_ASSEMBLY_COPY",
    )?;
    // Generated Assembly source owns include_bytes! paths into the exact
    // Product Bundle. Keep that source closure beside the scratch Assembly;
    // only the separately bridged browser frontend is published below.
    copy_tree(
        bundle_root,
        &staged_source_bundle,
        "RUSTY_DESKTOP_SOURCE_BUNDLE_COPY",
    )?;
    copy_tree(package_root, &staged_package, "RUSTY_DESKTOP_PACKAGE_COPY")?;
    copy_tree_without_bridge(bundle_root, &staged_frontend, "RUSTY_DESKTOP_FRONTEND_COPY")?;
    let bridge_source = native_bridge_source(manifest, wrapper);
    write_file(
        &staged_frontend.join("bridge.js"),
        bridge_source.as_bytes(),
        "RUSTY_DESKTOP_BRIDGE",
    )?;
    let bridge_check = workspace.join("bridge-check.mjs");
    write_file(
        &bridge_check,
        bridge_source.as_bytes(),
        "RUSTY_DESKTOP_BRIDGE_SYNTAX",
    )?;
    check_javascript_syntax(&bridge_check)?;
    let bootstrap_source = desktop_bootstrap_source();
    replace_regular_file_atomic(
        &staged_frontend.join("main.js"),
        bootstrap_source.as_bytes(),
        "RUSTY_DESKTOP_BOOTSTRAP",
    )?;
    let bootstrap_check = workspace.join("bootstrap-check.mjs");
    write_file(
        &bootstrap_check,
        bootstrap_source.as_bytes(),
        "RUSTY_DESKTOP_BOOTSTRAP_SYNTAX",
    )?;
    check_javascript_syntax(&bootstrap_check)?;
    let frontend_files = collect_tree(&staged_frontend, "RUSTY_DESKTOP_FRONTEND_HASH")?;
    let frontend_hashes = frontend_files
        .iter()
        .map(|(path, bytes)| (path.clone(), sha256_hex(bytes)))
        .collect::<Vec<_>>();
    let frontend_sha256 = sha256_tree(&frontend_files);
    write_file(
        &workspace.join("Cargo.toml"),
        tauri_cargo_source(manifest, wrapper, &build_workspace)?.as_bytes(),
        "RUSTY_DESKTOP_CARGO",
    )?;
    write_file(
        &workspace.join("build.rs"),
        b"fn main() { tauri_build::build(); }\n",
        "RUSTY_DESKTOP_BUILD_RS",
    )?;
    let config = tauri_config_source(wrapper)?;
    write_file(
        &workspace.join(DESKTOP_CONFIG),
        &config,
        "RUSTY_DESKTOP_CONFIG",
    )?;
    let capability = tauri_capability_source();
    write_file(
        &workspace.join(DESKTOP_CAPABILITY),
        capability.as_bytes(),
        "RUSTY_DESKTOP_CAPABILITY",
    )?;
    let icons = workspace.join("icons");
    create_directory(&icons, "RUSTY_DESKTOP_ICONS")?;
    write_file(
        &icons.join("icon.png"),
        ENGINE_FALLBACK_ICON_PNG,
        "RUSTY_DESKTOP_ENGINE_ICON",
    )?;
    write_file(
        &workspace.join("src/main.rs"),
        tauri_main_source_with_frontend_hashes(
            manifest,
            wrapper,
            &frontend_hashes,
            &frontend_sha256,
        )
        .as_bytes(),
        "RUSTY_DESKTOP_MAIN",
    )?;
    replace_build_workspace(&workspace, &build_workspace)?;
    patch_assembly_dependency_paths(&build_workspace.join("product-assembly"))?;
    let binary = build_tauri_release(&build_workspace)?;

    let final_stage = stage.join("release");
    create_directory(&final_stage, "RUSTY_DESKTOP_RELEASE_STAGE")?;
    copy_regular_file(
        &binary,
        &final_stage.join(DESKTOP_BINARY),
        "RUSTY_DESKTOP_BINARY",
    )?;
    copy_tree(
        &staged_frontend,
        &final_stage.join(DESKTOP_FRONTEND),
        "RUSTY_DESKTOP_RELEASE_FRONTEND",
    )?;
    copy_tree(
        &staged_package,
        &final_stage.join(DESKTOP_BASE_PACKAGE),
        "RUSTY_DESKTOP_RELEASE_PACKAGE",
    )?;
    write_file(
        &final_stage.join(DESKTOP_CONFIG),
        &config,
        "RUSTY_DESKTOP_RELEASE_CONFIG",
    )?;
    write_file(
        &final_stage.join(DESKTOP_CAPABILITY),
        capability.as_bytes(),
        "RUSTY_DESKTOP_RELEASE_CAPABILITY",
    )?;
    write_file(
        &final_stage.join(DESKTOP_LAUNCHER),
        launcher_source().as_bytes(),
        "RUSTY_DESKTOP_LAUNCHER",
    )?;
    write_file(
        &final_stage.join(DESKTOP_ENTRY_TEMPLATE),
        desktop_entry_template(wrapper).as_bytes(),
        "RUSTY_DESKTOP_ENTRY",
    )?;
    let policy = policy_source(wrapper)?;
    write_file(
        &final_stage.join(DESKTOP_POLICY),
        &policy,
        "RUSTY_DESKTOP_POLICY",
    )?;
    make_executable(&final_stage.join(DESKTOP_BINARY))?;
    make_executable(&final_stage.join(DESKTOP_LAUNCHER))?;

    let package_sha256 = packaged.package_sha256().to_owned();
    let assembly_bytes = assembly.json_bytes().map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_ASSEMBLY_RECEIPT",
            "assembly.json",
            error.to_string(),
        )
    })?;
    let assembly_sha256 = sha256_hex(&assembly_bytes);
    let config_sha256 = sha256_hex(&config);
    let policy_sha256 = sha256_hex(&policy);
    let receipt = make_receipt(
        &final_stage,
        manifest,
        wrapper,
        package_sha256.clone(),
        assembly_sha256.clone(),
        config_sha256,
        policy_sha256,
        frontend_sha256,
    )?;
    let receipt_bytes = encode_receipt(&receipt)?;
    write_file(
        &final_stage.join(DESKTOP_RECEIPT),
        &receipt_bytes,
        "RUSTY_DESKTOP_RECEIPT",
    )?;
    let expected = collect_tree(&final_stage, "RUSTY_DESKTOP_RELEASE_READBACK")?;
    Ok(StagedRelease {
        final_stage,
        expected,
        receipt,
        package_sha256,
        assembly_sha256,
    })
}

/// Verifies a relocated desktop directory using only its receipt and bytes.
pub(crate) fn verify_relocated(root: &Path) -> Result<DesktopPackage, DesktopError> {
    ensure_directory(root, "RUSTY_DESKTOP_READBACK", "desktop package")?;
    let actual = collect_tree(root, "RUSTY_DESKTOP_READBACK")?;
    let receipt_bytes = actual.get(DESKTOP_RECEIPT).ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_RECEIPT_MISSING",
            DESKTOP_RECEIPT,
            "published desktop release requires its exact receipt",
        )
    })?;
    let receipt: DesktopReceipt = serde_json::from_slice(receipt_bytes).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_RECEIPT_DECODE",
            DESKTOP_RECEIPT,
            error.to_string(),
        )
    })?;
    if receipt.artifact != DESKTOP_ARTIFACT {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_RECEIPT_ARTIFACT",
            DESKTOP_RECEIPT,
            "unsupported desktop receipt artifact",
        ));
    }
    if receipt
        .files
        .iter()
        .any(|entry| entry.path == DESKTOP_RECEIPT)
        || receipt
            .files
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_RECEIPT_ORDER",
            DESKTOP_RECEIPT,
            "desktop receipt entries must be strictly ordered and self-excluded",
        ));
    }
    let expected_paths = receipt
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<Vec<_>>();
    let actual_paths = actual
        .keys()
        .filter(|path| path.as_str() != DESKTOP_RECEIPT)
        .map(String::as_str)
        .collect::<Vec<_>>();
    if expected_paths != actual_paths {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_CLOSURE_SHAPE",
            root.display().to_string(),
            "desktop release tree does not exactly match its receipt",
        ));
    }
    for entry in &receipt.files {
        let Some(bytes) = actual.get(&entry.path) else {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_FILE_MISSING",
                &entry.path,
                "desktop receipt references a missing file",
            ));
        };
        if bytes.len() != entry.bytes || sha256_hex(bytes) != entry.sha256 {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_FILE_DRIFT",
                &entry.path,
                "desktop release file differs from its receipt",
            ));
        }
    }
    let frontend_files = subtree_files(&actual, DESKTOP_FRONTEND);
    if frontend_files.is_empty() || sha256_tree(&frontend_files) != receipt.frontend_sha256 {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_FRONTEND_DRIFT",
            DESKTOP_FRONTEND,
            "browser frontend bytes differ from the compile-time desktop frontend identity",
        ));
    }
    let config = actual.get(DESKTOP_CONFIG).ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_CONFIG_MISSING",
            DESKTOP_CONFIG,
            "Tauri config is required",
        )
    })?;
    if sha256_hex(config) != receipt.config_sha256 {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_CONFIG_DRIFT",
            DESKTOP_CONFIG,
            "Tauri config differs from the receipt",
        ));
    }
    let policy = actual.get(DESKTOP_POLICY).ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_POLICY_MISSING",
            DESKTOP_POLICY,
            "selected wrapper policy is required",
        )
    })?;
    if sha256_hex(policy) != receipt.policy_sha256 {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_POLICY_DRIFT",
            DESKTOP_POLICY,
            "selected wrapper policy differs from the receipt",
        ));
    }
    let selected_policy: DesktopPolicy = serde_json::from_slice(policy).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_POLICY_DECODE",
            DESKTOP_POLICY,
            error.to_string(),
        )
    })?;
    if selected_policy.wrapper_id != receipt.wrapper_id
        || selected_policy.wrapper_version != receipt.wrapper_version
        || selected_policy.application_id != receipt.application_id
        || selected_policy.storage_namespace != receipt.storage_namespace
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_RECEIPT_POLICY",
            DESKTOP_RECEIPT,
            "desktop receipt wrapper metadata differs from its selected policy",
        ));
    }
    if !actual.contains_key(DESKTOP_BINARY) || !actual.contains_key("frontend/index.html") {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_RUNTIME_CLOSURE",
            root.display().to_string(),
            "desktop release must contain one binary and one frontend index",
        ));
    }
    require_executable(
        &root.join(DESKTOP_BINARY),
        "RUSTY_DESKTOP_BINARY",
        "desktop binary",
    )?;
    require_executable(
        &root.join(DESKTOP_LAUNCHER),
        "RUSTY_DESKTOP_LAUNCHER",
        "desktop launcher",
    )?;
    let package =
        verify_product_package(&root.join(DESKTOP_BASE_PACKAGE)).map_err(package_error)?;
    if package.package_sha256() != receipt.package_sha256 {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_DRIFT",
            DESKTOP_BASE_PACKAGE,
            "adjacent base Product Package differs from the desktop receipt",
        ));
    }
    let package_identity = read_canonical_package_receipt(&root.join(DESKTOP_BASE_PACKAGE))?;
    if package_identity.artifact != "rusty.product.package"
        || package_identity.product != receipt.product
        || package_identity.assembly_sha256 != receipt.assembly_sha256
        || !package_identity
            .wrapper_policy
            .iter()
            .any(|policy| canonical_wrapper_matches_desktop_policy(policy, &selected_policy))
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_IDENTITY",
            DESKTOP_BASE_PACKAGE,
            "desktop receipt metadata is not bound to the canonical Product Package manifest and wrapper policy",
        ));
    }
    Ok(DesktopPackage {
        root: root.to_path_buf(),
        product: receipt.product,
        wrapper_id: receipt.wrapper_id,
        wrapper_version: receipt.wrapper_version,
        application_id: receipt.application_id,
        storage_namespace: receipt.storage_namespace,
        package_sha256: receipt.package_sha256,
        assembly_sha256: receipt.assembly_sha256,
        policy_sha256: receipt.policy_sha256,
        release_sha256: sha256_tree(&actual),
        files: actual.len(),
    })
}

/// Installs a verified release beneath a user-provided root. Current and
/// previous are complete immutable directory copies, not symlink aliases.
pub(crate) fn install(
    package_root: &Path,
    user_root: &Path,
    options: &InstallOptions,
) -> Result<InstalledDesktop, DesktopError> {
    validate_install_name(&options.launcher_name, "launcher_name")?;
    validate_install_name(&options.desktop_entry_name, "desktop_entry_name")?;
    ensure_directory(user_root, "RUSTY_DESKTOP_INSTALL_ROOT", "user install root")?;
    let releases = user_root.join("releases");
    let current = user_root.join("current");
    let previous = user_root.join("previous");
    let bin = user_root.join("bin");
    let applications = user_root.join("share/applications");
    let data_root = user_root.join("data");
    let current_existing = path_exists(&current)?;
    let previous_existing = path_exists(&previous)?;
    // Refuse unmanaged install state before validating or copying any new
    // release. This keeps an operator's data and existing files untouched on
    // an unsafe update attempt.
    if current_existing {
        verify_relocated(&current).map_err(|error| {
            DesktopError::new(
                "RUSTY_DESKTOP_INSTALL_UNMANAGED",
                current.display().to_string(),
                format!("current install is not a managed Rusty release: {error}"),
            )
        })?;
    }
    if previous_existing {
        verify_relocated(&previous).map_err(|error| {
            DesktopError::new(
                "RUSTY_DESKTOP_INSTALL_UNMANAGED",
                previous.display().to_string(),
                format!("previous install is not a managed Rusty release: {error}"),
            )
        })?;
    }
    let package = verify_relocated(package_root)?;
    for directory in [&releases, &bin, &applications, &data_root] {
        ensure_or_create_directory(directory, "RUSTY_DESKTOP_INSTALL_DIRECTORY")?;
    }
    let release = releases.join(package.release_sha256());
    let release_existing = path_exists(&release)?;
    if release_existing {
        let known = verify_relocated(&release)?;
        if known.release_sha256 != package.release_sha256() {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_INSTALL_RELEASE_DRIFT",
                release.display().to_string(),
                "existing immutable release has a different receipt; no files were changed",
            ));
        }
    }

    let existing_current = if current_existing {
        Some(verify_relocated(&current).map_err(|error| {
            DesktopError::new(
                "RUSTY_DESKTOP_INSTALL_UNMANAGED",
                current.display().to_string(),
                format!("current install is not a managed Rusty release: {error}"),
            )
        })?)
    } else {
        None
    };
    let launcher = bin.join(&options.launcher_name);
    let desktop_entry = applications.join(&options.desktop_entry_name);
    let template = read_regular_file(
        &package_root.join(DESKTOP_ENTRY_TEMPLATE),
        "RUSTY_DESKTOP_INSTALL_ENTRY",
        MAX_DESKTOP_TEXT_BYTES,
    )?;
    let template = String::from_utf8(template).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_INSTALL_ENTRY",
            DESKTOP_ENTRY_TEMPLATE,
            error.to_string(),
        )
    })?;
    let installed_exec = desktop_exec_argument(&user_root.join("current/launcher.sh"))?;
    let entry = template.replace("%INSTALL_ROOT%/launcher.sh", &installed_exec);
    let launcher_bytes = installed_launcher_source().as_bytes();
    let mut staging = InstallStaging::default();
    let staged_release = if release_existing {
        None
    } else {
        let stage = create_stage(&releases, INSTALL_STAGE_PREFIX)?;
        staging.track(stage.clone());
        copy_install_release(package_root, &stage, "RUSTY_DESKTOP_INSTALL_RELEASE")?;
        verify_relocated(&stage)?;
        Some(stage)
    };
    let current_needs_update = existing_current
        .as_ref()
        .is_none_or(|existing| existing.release_sha256() != package.release_sha256());
    let staged_current = if current_needs_update {
        let stage = create_stage(user_root, INSTALL_STAGE_PREFIX)?;
        staging.track(stage.clone());
        let source = staged_release.as_deref().unwrap_or(package_root);
        copy_install_release(source, &stage, "RUSTY_DESKTOP_INSTALL_CURRENT")?;
        verify_relocated(&stage)?;
        Some(stage)
    } else {
        None
    };
    let staged_launcher = prepare_managed_sidecar(
        &launcher,
        launcher_bytes,
        true,
        "RUSTY_DESKTOP_INSTALL_LAUNCHER",
    )?;
    if let Some(path) = staged_launcher.as_ref() {
        staging.track(path.clone());
    }
    let staged_entry = prepare_managed_sidecar(
        &desktop_entry,
        entry.as_bytes(),
        false,
        "RUSTY_DESKTOP_INSTALL_ENTRY",
    )?;
    if let Some(path) = staged_entry.as_ref() {
        staging.track(path.clone());
    }
    commit_install_transaction(
        &release,
        staged_release.as_deref(),
        &current,
        &previous,
        staged_current.as_deref(),
        current_existing,
        previous_existing,
        &launcher,
        staged_launcher.as_deref(),
        &desktop_entry,
        staged_entry.as_deref(),
    )?;
    staging.disarm();
    Ok(InstalledDesktop {
        root: user_root.to_path_buf(),
        release,
        launcher,
        desktop_entry,
        data_root,
        application_id: package.application_id().to_owned(),
        storage_namespace: package.storage_namespace().to_owned(),
    })
}

#[derive(Default)]
struct InstallStaging {
    paths: Vec<PathBuf>,
    armed: bool,
}

impl InstallStaging {
    fn track(&mut self, path: PathBuf) {
        self.armed = true;
        self.paths.push(path);
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for InstallStaging {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for path in self.paths.iter().rev() {
            let _ = remove_install_path(path);
        }
    }
}

fn path_exists(path: &Path) -> Result<bool, DesktopError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(
            "RUSTY_DESKTOP_INSTALL_READ",
            path.display().to_string(),
            error,
        )),
    }
}

fn prepare_managed_sidecar(
    destination: &Path,
    bytes: &[u8],
    executable: bool,
    code: &'static str,
) -> Result<Option<PathBuf>, DesktopError> {
    match fs::symlink_metadata(destination) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(DesktopError::new(
                    code,
                    destination.display().to_string(),
                    "existing install file is unmanaged",
                ));
            }
            if read_regular_file(destination, code, MAX_DESKTOP_FILE_BYTES)? != bytes {
                return Err(DesktopError::new(
                    code,
                    destination.display().to_string(),
                    "existing install file differs from the managed template",
                ));
            }
            if executable {
                require_executable(destination, code, "installed launcher")?;
            }
            Ok(None)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = destination.parent().ok_or_else(|| {
                DesktopError::new(
                    code,
                    destination.display().to_string(),
                    "install file has no parent",
                )
            })?;
            let stage = fresh_path(parent, ".rusty-product-install-file-")?;
            write_file(&stage, bytes, code)?;
            if executable {
                make_executable(&stage)?;
            }
            Ok(Some(stage))
        }
        Err(error) => Err(io_error(code, destination.display().to_string(), error)),
    }
}

#[allow(clippy::too_many_arguments)]
fn commit_install_transaction(
    release: &Path,
    staged_release: Option<&Path>,
    current: &Path,
    previous: &Path,
    staged_current: Option<&Path>,
    current_existing: bool,
    previous_existing: bool,
    launcher: &Path,
    staged_launcher: Option<&Path>,
    desktop_entry: &Path,
    staged_entry: Option<&Path>,
) -> Result<(), DesktopError> {
    let parent = current.parent().unwrap_or_else(|| Path::new("."));
    let previous_backup = if staged_current.is_some() && current_existing && previous_existing {
        Some(fresh_path(parent, ".rusty-product-previous-backup-")?)
    } else {
        None
    };
    let mut release_published = false;
    let mut previous_backed_up = false;
    let mut current_moved = false;
    let mut current_published = false;
    let mut launcher_published = false;
    let mut entry_published = false;

    let operation = (|| {
        if let Some(stage) = staged_release {
            fs::rename(stage, release).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_INSTALL_RELEASE",
                    release.display().to_string(),
                    error,
                )
            })?;
            release_published = true;
        }
        if let Some(stage) = staged_current {
            if let Some(backup) = previous_backup.as_deref() {
                fs::rename(previous, backup).map_err(|error| {
                    io_error(
                        "RUSTY_DESKTOP_INSTALL_ROTATE",
                        previous.display().to_string(),
                        error,
                    )
                })?;
                previous_backed_up = true;
            }
            if current_existing {
                fs::rename(current, previous).map_err(|error| {
                    io_error(
                        "RUSTY_DESKTOP_INSTALL_ROTATE",
                        current.display().to_string(),
                        error,
                    )
                })?;
                current_moved = true;
            }
            fs::rename(stage, current).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_INSTALL_CURRENT",
                    current.display().to_string(),
                    error,
                )
            })?;
            current_published = true;
        }
        if let Some(stage) = staged_launcher {
            fs::hard_link(stage, launcher).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_INSTALL_LAUNCHER",
                    launcher.display().to_string(),
                    error,
                )
            })?;
            launcher_published = true;
        }
        if let Some(stage) = staged_entry {
            fs::hard_link(stage, desktop_entry).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_INSTALL_ENTRY",
                    desktop_entry.display().to_string(),
                    error,
                )
            })?;
            entry_published = true;
        }
        Ok::<(), DesktopError>(())
    })();

    if let Err(error) = operation {
        let mut rollback = Vec::new();
        if entry_published {
            collect_install_rollback(&mut rollback, desktop_entry, fs::remove_file(desktop_entry));
        }
        if launcher_published {
            collect_install_rollback(&mut rollback, launcher, fs::remove_file(launcher));
        }
        if current_published {
            collect_install_rollback(&mut rollback, current, fs::remove_dir_all(current));
        }
        if current_moved {
            collect_install_rollback(&mut rollback, current, fs::rename(previous, current));
        }
        if previous_backed_up {
            if let Some(backup) = previous_backup.as_deref() {
                collect_install_rollback(&mut rollback, previous, fs::rename(backup, previous));
            }
        }
        if release_published {
            collect_install_rollback(&mut rollback, release, fs::remove_dir_all(release));
        }
        return Err(with_install_rollback(error, rollback));
    }

    if let Some(backup) = previous_backup.as_deref() {
        remove_tree(backup).map_err(|cleanup| {
            DesktopError::new(
                "RUSTY_DESKTOP_INSTALL_CLEANUP",
                backup.display().to_string(),
                format!("install committed but obsolete previous-release backup could not be removed: {cleanup}"),
            )
        })?;
    }
    Ok(())
}

fn collect_install_rollback(failures: &mut Vec<String>, path: &Path, result: io::Result<()>) {
    if let Err(error) = result {
        failures.push(format!("{}: {error}", path.display()));
    }
}

fn with_install_rollback(error: DesktopError, failures: Vec<String>) -> DesktopError {
    if failures.is_empty() {
        error
    } else {
        DesktopError::new(
            "RUSTY_DESKTOP_INSTALL_ROLLBACK",
            error.path(),
            format!(
                "{}; rollback also failed: {}",
                error.detail(),
                failures.join("; ")
            ),
        )
    }
}

fn remove_install_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            fs::remove_dir_all(path)
        }
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
fn rotate_install_releases(
    current: &Path,
    previous: &Path,
    staged_current: &Path,
    previous_exists: bool,
) -> Result<(), DesktopError> {
    let previous_backup = fresh_path(
        previous.parent().unwrap_or_else(|| Path::new(".")),
        ".rusty-product-previous-backup-",
    )?;
    let mut moved_previous = false;
    let mut moved_current = false;
    let mut installed_current = false;
    let operation = (|| {
        if previous_exists {
            fs::rename(previous, &previous_backup).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_INSTALL_ROTATE",
                    previous.display().to_string(),
                    error,
                )
            })?;
            moved_previous = true;
        }
        fs::rename(current, previous).map_err(|error| {
            io_error(
                "RUSTY_DESKTOP_INSTALL_ROTATE",
                current.display().to_string(),
                error,
            )
        })?;
        moved_current = true;
        fs::rename(staged_current, current).map_err(|error| {
            io_error(
                "RUSTY_DESKTOP_INSTALL_ROTATE",
                current.display().to_string(),
                error,
            )
        })?;
        installed_current = true;
        Ok::<(), DesktopError>(())
    })();
    if let Err(error) = operation {
        if installed_current {
            let _ = remove_tree(current);
        }
        if moved_current {
            let _ = fs::rename(previous, current);
        }
        if moved_previous {
            let _ = fs::rename(&previous_backup, previous);
        }
        let _ = remove_tree(staged_current);
        return Err(error);
    }
    if moved_previous {
        remove_tree(&previous_backup)?;
    }
    Ok(())
}

fn fresh_path(parent: &Path, prefix: &str) -> Result<PathBuf, DesktopError> {
    for _ in 0..64 {
        let counter = STAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{prefix}{}-{counter}", std::process::id()));
        match fs::symlink_metadata(&path) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(path),
            Err(error) => {
                return Err(io_error(
                    "RUSTY_DESKTOP_STAGE",
                    path.display().to_string(),
                    error,
                ))
            }
        }
    }
    Err(DesktopError::new(
        "RUSTY_DESKTOP_STAGE",
        parent.display().to_string(),
        "could not allocate a bounded unique path",
    ))
}

fn read_manifest(product_root: &Path) -> Result<ProductManifest, DesktopError> {
    let path = product_root.join("rusty.toml");
    let bytes = read_regular_file(&path, "RUSTY_DESKTOP_MANIFEST", 65_536)?;
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        DesktopError::new("RUSTY_DESKTOP_MANIFEST", "rusty.toml", error.to_string())
    })?;
    decode_product_manifest(text).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_MANIFEST",
            "rusty.toml",
            format!("manifest admission failed: {error}"),
        )
    })
}

fn read_canonical_package_receipt(
    package_root: &Path,
) -> Result<CanonicalPackageReceipt, DesktopError> {
    let path = package_root.join("package.json");
    let bytes = read_regular_file(
        &path,
        "RUSTY_DESKTOP_PACKAGE_RECEIPT",
        MAX_DESKTOP_TEXT_BYTES,
    )?;
    let receipt = serde_json::from_slice(&bytes).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_RECEIPT",
            path.display().to_string(),
            error.to_string(),
        )
    })?;
    Ok(receipt)
}

fn canonical_wrapper_matches(policy: &CanonicalWrapperPolicy, wrapper: &SelectedWrapper) -> bool {
    policy.id == wrapper.id
        && policy.kind == "tauri"
        && policy.version == wrapper.version
        && policy.application_id == wrapper.application_id
        && policy.title == wrapper.title
        && policy.window_width == wrapper.width
        && policy.window_height == wrapper.height
        && policy.resizable == wrapper.resizable
        && policy.permissions == wrapper.permissions
        && policy.storage_namespace == wrapper.storage_namespace
        && policy.release_channel == wrapper.release_channel
        && policy.singleton == wrapper.singleton
}

fn canonical_wrapper_matches_desktop_policy(
    policy: &CanonicalWrapperPolicy,
    selected: &DesktopPolicy,
) -> bool {
    policy.id == selected.wrapper_id
        && policy.kind == "tauri"
        && policy.version == selected.wrapper_version
        && policy.application_id == selected.application_id
        && policy.title == selected.title
        && policy.window_width == selected.window_width
        && policy.window_height == selected.window_height
        && policy.resizable == selected.resizable
        && policy.permissions == selected.permissions
        && policy.storage_namespace == selected.storage_namespace
        && policy.release_channel == selected.release_channel
        && policy.singleton == selected.singleton
}

fn validate_canonical_package_identity(
    package: &CanonicalPackageReceipt,
    manifest: &ProductManifest,
    wrapper: &SelectedWrapper,
    assembly_sha256: &str,
) -> Result<(), DesktopError> {
    if package.artifact != "rusty.product.package"
        || package.product != manifest.product_id()
        || package.assembly_sha256 != assembly_sha256
        || package.entries.is_empty()
        || package
            .entries
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
        || package
            .entries
            .iter()
            .any(|entry| entry.path.is_empty() || entry.sha256.len() != 64)
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_IDENTITY",
            "product-package/package.json",
            "canonical Product Package manifest identity differs from the verified Assembly or has invalid entries",
        ));
    }
    if !package
        .wrapper_policy
        .iter()
        .any(|policy| canonical_wrapper_matches(policy, wrapper))
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_PACKAGE_POLICY",
            "product-package/package.json",
            "selected Tauri wrapper is not represented by the canonical Product Package policy",
        ));
    }
    Ok(())
}

fn select_tauri_wrapper(
    manifest: &ProductManifest,
    explicit_id: Option<&str>,
) -> Result<SelectedWrapper, DesktopError> {
    let tauri = manifest
        .wrappers()
        .iter()
        .filter(|wrapper| wrapper.kind() == WrapperKind::Tauri)
        .collect::<Vec<_>>();
    let selected = if let Some(id) = explicit_id {
        let Some(wrapper) = manifest
            .wrappers()
            .iter()
            .find(|wrapper| wrapper.id() == id)
        else {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_WRAPPER_SELECTION",
                "wrappers",
                format!("explicit Tauri wrapper {id} is not declared"),
            ));
        };
        if wrapper.kind() != WrapperKind::Tauri {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_WRAPPER_KIND",
                format!("wrappers/{id}"),
                "desktop realization only supports a selected Tauri wrapper",
            ));
        }
        wrapper
    } else if tauri.len() == 1 {
        tauri[0]
    } else {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_WRAPPER_SELECTION",
            "wrappers",
            "desktop realization requires exactly one manifest Tauri wrapper or an explicit wrapper id",
        ));
    };
    Ok(SelectedWrapper::from_declaration(selected))
}

fn tauri_cargo_source(
    manifest: &ProductManifest,
    wrapper: &SelectedWrapper,
    workspace: &Path,
) -> Result<String, DesktopError> {
    let package_name = generated_package_name(manifest.product_id());
    let engine_dependency = relative_path(
        workspace,
        &engine_checkout_root().join("rust/crates/rusty-engine"),
    )?;
    let plugin = if wrapper.singleton {
        "\ntauri-plugin-single-instance = \"2.4.3\""
    } else {
        ""
    };
    Ok(format!(
        "[package]\nname = \"rusty-product-desktop\"\nversion = \"{}\"\nedition = \"2021\"\nbuild = \"build.rs\"\n\n[dependencies]\nrusty_product = {{ package = \"{}\", path = \"product-assembly\" }}\nrusty-engine = {{ path = {} }}\nserde = \"1\"\nserde_json = \"1\"\nsha2 = \"0.10\"\ntauri = {{ version = \"2.11.5\", features = [\"wry\"] }}{}\n\n[build-dependencies]\ntauri-build = \"2.6.3\"\n\n[workspace]\nexclude = [\"product-assembly\"]\n",
        wrapper.version,
        package_name,
        rust_string(&engine_dependency),
        plugin,
    ))
}

fn tauri_config_source(wrapper: &SelectedWrapper) -> Result<Vec<u8>, DesktopError> {
    #[derive(Serialize)]
    struct Build<'a> {
        #[serde(rename = "frontendDist")]
        frontend_dist: &'a str,
        #[serde(rename = "devUrl")]
        dev_url: Option<&'a str>,
    }
    #[derive(Serialize)]
    struct Security<'a> {
        csp: &'a str,
    }
    #[derive(Serialize)]
    struct App<'a> {
        #[serde(rename = "withGlobalTauri")]
        with_global_tauri: bool,
        windows: Vec<serde_json::Value>,
        security: Security<'a>,
    }
    #[derive(Serialize)]
    struct Bundle<'a> {
        active: bool,
        icon: Vec<&'a str>,
        resources: Vec<&'a str>,
        targets: Vec<&'a str>,
    }
    #[derive(Serialize)]
    struct Config<'a> {
        #[serde(rename = "$schema")]
        schema: &'a str,
        #[serde(rename = "productName")]
        product_name: &'a str,
        version: &'a str,
        identifier: &'a str,
        build: Build<'a>,
        app: App<'a>,
        bundle: Bundle<'a>,
    }
    let config = Config {
        schema: "https://schema.tauri.app/config/2",
        product_name: &wrapper.title,
        version: &wrapper.version,
        identifier: &wrapper.application_id,
        build: Build {
            frontend_dist: "frontend",
            dev_url: None,
        },
        app: App {
            with_global_tauri: true,
            // Window creation is intentionally in generated main.rs after
            // resource and receipt preflight, not in tauri.conf.json.
            windows: Vec::new(),
            security: Security {
                csp: "default-src 'self'; connect-src 'self' asset: tauri:; img-src 'self' asset: data:; style-src 'self' 'unsafe-inline'; script-src 'self';",
            },
        },
        bundle: Bundle {
            active: true,
            // Tauri's context generator requires an RGBA PNG even without
            // authored product artwork. The host writes this fixed Engine
            // fallback into the scratch workspace only.
            icon: vec!["icons/icon.png"],
            resources: vec!["product-package/**/*"],
            targets: vec!["appimage"],
        },
    };
    serde_json::to_vec_pretty(&config).map_err(|error| {
        DesktopError::new("RUSTY_DESKTOP_CONFIG", DESKTOP_CONFIG, error.to_string())
    })
}

fn tauri_capability_source() -> &'static str {
    r#"{
  "identifier": "rusty-product-main",
  "description": "The generated main WebView may observe typed Rust runtime outputs.",
  "windows": ["main"],
  "permissions": [
    "core:event:allow-listen",
    "core:event:allow-unlisten"
  ]
}
"#
}

fn native_bridge_source(manifest: &ProductManifest, wrapper: &SelectedWrapper) -> String {
    let lifecycle = match manifest.lifecycle() {
        product_model::LifecycleMode::Realtime => "realtime",
        product_model::LifecycleMode::Demand => "demand",
        product_model::LifecycleMode::External => "external",
    };
    let projection = match (
        manifest.ui_projection_stream(),
        manifest.ui_projection_contract(),
    ) {
        (Some(stream), Some(contract)) => format!(
            "uiProjection: {{ expectedStream: {}, expectedContract: {} }},",
            json_string(stream),
            json_string(contract)
        ),
        _ => "uiProjection: undefined,".to_owned(),
    };
    format!(
        r#"import {{ createProductBrowserRuntimeTransport }} from './engine/product-browser-host.js';

const tauri = globalThis.__TAURI__;
if (tauri === undefined || tauri.core === undefined || tauri.event === undefined) throw new Error('Rusty Tauri global is unavailable');
const invoke = tauri.core.invoke;
const listen = tauri.event.listen;
const PRODUCT_WRAPPER_ID = {};
let outputListenerReady = Promise.resolve();
let terminalFailureListenerReady = Promise.resolve();

function command(name, payload, operation) {{
  if (document.body !== null) document.body.dataset.rustyLastRuntimeCommand = name;
  return invoke(name, payload).then((result) => {{
    if (result === null || typeof result !== 'object' || Array.isArray(result)) {{
      throw new Error(operation + ' returned an invalid typed result');
    }}
    if (document.body !== null) {{
      document.body.dataset.rustyLastRuntimeAccepted = String(result?.accepted);
      document.body.dataset.rustyLastRuntimeCount = String(result?.count ?? '');
      document.body.dataset.rustyLastRuntimeDiagnostic = String(result?.diagnostic ?? '');
    }}
    return result;
  }});
}}
function subscribeOutputs(listener) {{
  let disposed = false;
  const unlisten = [];
  const subscribeOutput = listen('rusty-runtime-output', (event) => {{
    if (document.body !== null) document.body.dataset.rustyLastRuntimeOutput = String(event.payload?.kind ?? 'unknown');
    if (!disposed) listener(event.payload);
  }}).then((remove) => {{ if (disposed) remove(); else unlisten.push(remove); }});
  const subscribeProgress = listen('rusty-runtime-progress', (event) => {{
    const payload = event.payload;
    const valid = payload !== null
      && typeof payload === 'object'
      && !Array.isArray(payload)
      && Object.keys(payload).length === 2
      && payload.kind === 'runtime-progress'
      && payload.owner === 'rust-host';
    const progress = Object.freeze(valid
      ? {{ kind: 'runtime-progress', owner: 'rust-host' }}
      : {{ kind: 'runtime-progress', owner: 'invalid' }});
    if (document.body !== null) document.body.dataset.rustyLastRuntimeOutput = progress.kind;
    if (!disposed) listener(progress);
  }}).then((remove) => {{ if (disposed) remove(); else unlisten.push(remove); }});
  outputListenerReady = Promise.all([subscribeOutput, subscribeProgress]).then(() => undefined);
  return () => {{ disposed = true; for (const remove of unlisten) remove(); }};
}}
function subscribeTerminalFailures(listener) {{
  let disposed = false;
  let unlisten = null;
  terminalFailureListenerReady = listen('rusty-runtime-terminal-failure', (event) => {{
    const payload = event.payload;
    const valid = payload !== null
      && typeof payload === 'object'
      && !Array.isArray(payload)
      && payload.kind === 'runtime-failure'
      && typeof payload.diagnostic === 'string'
      && payload.diagnostic.length > 0
      && new TextEncoder().encode(payload.diagnostic).byteLength <= 512;
    if (!disposed) listener(valid
      ? payload
      : {{ kind: 'runtime-failure', diagnostic: 'native runtime failure event was malformed' }});
  }}).then((remove) => {{ if (disposed) remove(); else unlisten = remove; }});
  return () => {{ disposed = true; if (unlisten !== null) unlisten(); }};
}}

export function createProductBridge() {{
  const transport = createProductBrowserRuntimeTransport({{
    lifecycle: (operation) => operation.kind === 'start'
      ? Promise.all([outputListenerReady, terminalFailureListenerReady])
        .then(() => command('runtime_lifecycle', {{ operation: operation.kind }}, 'lifecycle:' + operation.kind))
      : command('runtime_lifecycle', {{ operation: operation.kind }}, 'lifecycle:' + operation.kind),
    input: (batch) => command('runtime_input', {{ batch }}, 'input'),
    advanceRealtime: (observedTimeNs) => command('advance_realtime', {{ observedTimeNs }}, 'advance-realtime'),
    admitDemandStep: () => command('admit_demand_step', {{}}, 'admit-demand-step'),
    admitExternalStep: (step) => command('admit_external_step', {{ step }}, 'admit-external-step'),
    completeTimeline: (completion) => command('complete_timeline', {{ completion }}, 'complete-timeline'),
    subscribeTerminalFailures,
    subscribeOutputs,
    dispose: () => command('runtime_shutdown', {{}}, 'shutdown').then(() => undefined),
  }});
  return {{ transport, lifecycleMode: {}, realtimeAdvanceOwner: 'rust-host', {} runtimeInput: {{ maximumPointerDelta: 32, maximumWheelDelta: 64 }} }};
}}

void PRODUCT_WRAPPER_ID;
"#,
        json_string(&wrapper.id),
        json_string(lifecycle),
        projection,
    )
}

fn desktop_bootstrap_source() -> &'static str {
    r#"const MAX_STARTUP_ERROR_BYTES = 1024;

function renderDesktopStartupFailure(error) {
  const detail = error instanceof Error ? error.message : String(error);
  const message = detail.slice(0, MAX_STARTUP_ERROR_BYTES);
  const body = document.body;
  if (body === null) return;
  body.dataset.desktopStartupError = message;
  body.setAttribute('data-desktop-startup-error', message);
  const root = document.querySelector('#application');
  if (root === null) return;
  root.replaceChildren();
  const failure = document.createElement('pre');
  failure.dataset.desktopStartupError = '';
  failure.dataset.startupError = '';
  failure.setAttribute('data-startup-error', '');
  failure.setAttribute('role', 'alert');
  failure.textContent = 'Rusty Product failed to start: ' + message;
  root.append(failure);
}

try {
  const [
    { mountProductBrowserHost },
    { createProductBridge },
    { mountProductUi },
  ] = await Promise.all([
    import('./engine/product-browser-host.js'),
    import('./bridge.js'),
    import('./ui/main.js'),
  ]);
  const root = document.querySelector('#application');
  if (root === null) throw new Error('generated Product Browser Host root is missing');
  const bridge = createProductBridge();
  const host = await mountProductBrowserHost({
    root,
    transport: bridge.transport,
    lifecycleMode: bridge.lifecycleMode,
    realtimeAdvanceOwner: bridge.realtimeAdvanceOwner,
    initialInteractionMode: 'gameplay',
    mountUi: mountProductUi,
    uiProjection: bridge.uiProjection,
    runtimeInput: bridge.runtimeInput,
  });
  document.body.dataset.rustyProductReady = 'true';
  void host;
} catch (error) {
  renderDesktopStartupFailure(error);
}
"#
}

#[cfg(test)]
fn tauri_main_source(manifest: &ProductManifest, wrapper: &SelectedWrapper) -> String {
    tauri_main_source_with_frontend_hashes(manifest, wrapper, &[], "")
}

fn tauri_main_source_with_frontend_hashes(
    manifest: &ProductManifest,
    wrapper: &SelectedWrapper,
    frontend_hashes: &[(String, String)],
    frontend_sha256: &str,
) -> String {
    let singleton_plugin = if wrapper.singleton {
        "    builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {\n        if let Some(window) = app.get_webview_window(\"main\") {\n            let _ = window.unminimize();\n            let _ = window.show();\n            let _ = window.set_focus();\n            if let Err(error) = write_activation_receipt(app) {\n                eprintln!(\"Rusty Product activation receipt failed: {error}\");\n                if let Err(display_error) = show_startup_failure(app, &format!(\"activation receipt: {error}\")) {\n                    eprintln!(\"Rusty Product activation failure display failed: {display_error}\");\n                }\n            }\n        }\n    }));\n"
    } else {
        ""
    };
    let runtime_mode = match manifest.lifecycle() {
        product_model::LifecycleMode::Realtime => "realtime",
        product_model::LifecycleMode::Demand => "demand",
        product_model::LifecycleMode::External => "external",
    };
    let mut source = r##"#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use rusty_engine::product_dev_host::{
    CanonicalU64, ProductDevLifecycleOperation, ProductDevRuntimeReceipt,
    ProductDevOperationOwner,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindowBuilder, WebviewUrl};

const PACKAGE_RECEIPT: &[u8] = include_bytes!("../product-package/package.json");
const ASSEMBLY_RECEIPT: &[u8] = include_bytes!("../product-assembly/assembly.json");
const FRONTEND_INDEX: &[u8] = include_bytes!("../frontend/index.html");
const FRONTEND_TREE_SHA256: &str = "__FRONTEND_TREE_SHA256__";
const FRONTEND_FILE_HASHES: &[(&str, &str)] = &[
__FRONTEND_FILE_HASHES__];
const RUNTIME_MODE: &str = "__RUNTIME_MODE__";
const RUSTY_PRODUCT_ACTIVATION_RECEIPT: &str = "RUSTY_PRODUCT_ACTIVATION_RECEIPT";
const ACTIVATION_RECEIPT_FILENAME: &str = "activation.json";
const MAX_PACKAGE_RECEIPT_ENTRIES: usize = 16_512;
const MAX_PACKAGE_ENTRY_BYTES: usize = 256 * 1024 * 1024;
const MAX_FRONTEND_FILES: usize = 16_384;
const MAX_FRONTEND_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_FRONTEND_TOTAL_BYTES: usize = 256 * 1024 * 1024;

type GeneratedRuntime = rusty_product::product::GeneratedProductDevRuntime;

#[derive(Clone)]
struct DesktopRuntime {
    session: Arc<ProductDevOperationOwner<GeneratedRuntime>>,
    stop: Arc<AtomicBool>,
    ticker_started: Arc<AtomicBool>,
    ticker_control: Arc<Mutex<TickerControl>>,
    ticker_gate: Arc<Mutex<()>>,
}

struct TickerControl {
    active: bool,
}

fn lock_ticker_control(control: &Mutex<TickerControl>) -> std::sync::MutexGuard<'_, TickerControl> {
    control.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn ticker_is_active(control: &Mutex<TickerControl>) -> bool {
    lock_ticker_control(control).active
}

fn deactivate_ticker(control: &Mutex<TickerControl>) -> bool {
    let mut ticker = lock_ticker_control(control);
    let was_active = ticker.active;
    ticker.active = false;
    was_active
}

fn activate_ticker(control: &Mutex<TickerControl>) {
    lock_ticker_control(control).active = true;
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LifecycleOperation {
    Start,
    Pause,
    Resume,
    Restart,
    Shutdown,
    ReportFault,
}

impl LifecycleOperation {
    const fn engine(self) -> ProductDevLifecycleOperation {
        match self {
            Self::Start => ProductDevLifecycleOperation::Start,
            Self::Pause => ProductDevLifecycleOperation::Pause,
            Self::Resume => ProductDevLifecycleOperation::Resume,
            Self::Restart => ProductDevLifecycleOperation::Restart,
            Self::Shutdown => ProductDevLifecycleOperation::Shutdown,
            Self::ReportFault => ProductDevLifecycleOperation::ReportFault,
        }
    }
}

fn error_text(error: rusty_engine::product_dev_host::ProductDevRuntimeError) -> String {
    format!("{}: {}", error.code(), error.diagnostic())
}

fn encode_receipt<T: Serialize>(receipt: ProductDevRuntimeReceipt<T>) -> Result<Value, String> {
    let (result, outputs) = receipt.into_parts();
    Ok(json!({ "result": result, "outputs": outputs }))
}

fn publish_receipt<T: Serialize>(
    app: &AppHandle,
    receipt: ProductDevRuntimeReceipt<T>,
) -> Result<Value, String> {
    let value = encode_receipt(receipt)?;
    publish(app, &value)?;
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "runtime receipt result is missing".to_owned())
}

fn publish(app: &AppHandle, value: &Value) -> Result<(), String> {
    let outputs = value
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime receipt outputs are not an array".to_owned())?;
    for output in outputs {
        app.emit("rusty-runtime-output", output)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn bounded_terminal_diagnostic(stage: &str, error: &str) -> String {
    let mut diagnostic = format!("{stage}: ");
    for character in error.chars() {
        if diagnostic.len().saturating_add(character.len_utf8()) > 512 {
            break;
        }
        diagnostic.push(character);
    }
    if diagnostic.len() == stage.len() + 2 {
        diagnostic.push_str("runtime operation failed");
    }
    diagnostic
}

fn publish_terminal_failure(app: &AppHandle, stage: &str, error: &str) {
    let payload = json!({
        "kind": "runtime-failure",
        "diagnostic": bounded_terminal_diagnostic(stage, error),
    });
    if let Err(emit_error) = app.emit("rusty-runtime-terminal-failure", payload) {
        eprintln!("Rusty Product runtime terminal failure event could not be published: {emit_error}");
    }
}

fn publish_realtime_progress(app: &AppHandle) -> Result<(), String> {
    app.emit(
        "rusty-runtime-progress",
        json!({ "kind": "runtime-progress", "owner": "rust-host" }),
    )
    .map_err(|error| error.to_string())
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PackageReceipt {
    artifact: String,
    product: String,
    assembly_sha256: String,
    entries: Vec<PackageEntry>,
    wrapper_policy: Vec<Value>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PackageEntry {
    path: String,
    bytes: usize,
    sha256: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn package_entry_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty() {
        return Err("package receipt contains an empty path".to_owned());
    }
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("package receipt path is not relative and normal: {relative}"));
    }
    Ok(root.join(path))
}

fn preflight_package(root: &Path) -> Result<(), String> {
    let root_metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("package root metadata failed: {error}"))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("bundled Product Package root is not a regular directory".to_owned());
    }
    let receipt_path = root.join("package.json");
    let receipt_bytes = fs::read(&receipt_path)
        .map_err(|error| format!("package receipt read failed: {error}"))?;
    if receipt_bytes != PACKAGE_RECEIPT {
        return Err("bundled Product Package receipt differs from the admitted receipt".to_owned());
    }
    let receipt: PackageReceipt = serde_json::from_slice(&receipt_bytes)
        .map_err(|error| format!("package receipt decode failed: {error}"))?;
    if receipt.artifact != "rusty.product.package"
        || receipt.product.is_empty()
        || receipt.assembly_sha256.len() != 64
        || receipt.entries.len() > MAX_PACKAGE_RECEIPT_ENTRIES
        || receipt.wrapper_policy.is_empty()
    {
        return Err("bundled Product Package receipt identity or bounds are invalid".to_owned());
    }
    let mut seen = BTreeSet::new();
    for entry in receipt.entries {
        if !seen.insert(entry.path.clone()) {
            return Err(format!("package receipt repeats entry {}", entry.path));
        }
        let path = package_entry_path(root, &entry.path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("package entry {} metadata failed: {error}", entry.path))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!("package entry {} is not a regular file", entry.path));
        }
        let bytes = usize::try_from(metadata.len())
            .map_err(|_| format!("package entry {} length is not bounded", entry.path))?;
        if bytes > MAX_PACKAGE_ENTRY_BYTES || bytes != entry.bytes {
            return Err(format!("package entry {} byte count differs", entry.path));
        }
        let contents = fs::read(&path)
            .map_err(|error| format!("package entry {} read failed: {error}", entry.path))?;
        if contents.len() != bytes || sha256_hex(&contents) != entry.sha256 {
            return Err(format!("package entry {} hash differs", entry.path));
        }
    }
    Ok(())
}

fn collect_frontend_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    total: &mut usize,
) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("frontend directory read failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("frontend directory entry failed: {error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("frontend entry metadata failed: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("frontend resource closure contains a symlink".to_owned());
        }
        if metadata.is_dir() {
            collect_frontend_files(root, &path, files, total)?;
            continue;
        }
        if !metadata.is_file() {
            return Err("frontend resource closure contains a non-file".to_owned());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "frontend resource path escaped its root".to_owned())?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.is_empty() || relative.split('/').count() > 64 {
            return Err("frontend resource path is outside its bounds".to_owned());
        }
        let bytes = fs::read(&path).map_err(|error| format!("frontend resource read failed: {error}"))?;
        if bytes.len() > MAX_FRONTEND_FILE_BYTES {
            return Err(format!("frontend resource {relative} exceeds its byte bound"));
        }
        *total = total
            .checked_add(bytes.len())
            .ok_or_else(|| "frontend resource byte accounting overflowed".to_owned())?;
        if *total > MAX_FRONTEND_TOTAL_BYTES {
            return Err("frontend resource closure exceeds its total byte bound".to_owned());
        }
        if files.insert(relative, bytes).is_some() || files.len() > MAX_FRONTEND_FILES {
            return Err("frontend resource closure contains duplicate or excessive files".to_owned());
        }
    }
    Ok(())
}

fn frontend_tree_sha256(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn preflight_frontend(root: &Path) -> Result<(), String> {
    let frontend = root.join("frontend");
    let metadata = fs::symlink_metadata(&frontend)
        .map_err(|error| format!("frontend root metadata failed: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("adjacent frontend root is not a regular directory".to_owned());
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    collect_frontend_files(&frontend, &frontend, &mut files, &mut total)?;
    if files.len() != FRONTEND_FILE_HASHES.len() {
        return Err("adjacent frontend file count differs from the embedded identity".to_owned());
    }
    for &(path, expected) in FRONTEND_FILE_HASHES {
        let bytes = files
            .get(path)
            .ok_or_else(|| format!("adjacent frontend file is missing: {path}"))?;
        if sha256_hex(bytes) != expected {
            return Err(format!("adjacent frontend file differs from its embedded identity: {path}"));
        }
    }
    if frontend_tree_sha256(&files) != FRONTEND_TREE_SHA256 {
        return Err("adjacent frontend tree differs from its embedded identity".to_owned());
    }
    Ok(())
}

fn preflight(app: &AppHandle) -> Result<(), String> {
    let package: Value = serde_json::from_slice(PACKAGE_RECEIPT)
        .map_err(|error| format!("package receipt decode: {error}"))?;
    let assembly: Value = serde_json::from_slice(ASSEMBLY_RECEIPT)
        .map_err(|error| format!("assembly receipt decode: {error}"))?;
    if package.get("artifact").and_then(Value::as_str) != Some("rusty.product.package")
        || assembly.get("artifact").and_then(Value::as_str) != Some("rusty.product.assembly")
        || FRONTEND_INDEX.is_empty()
    {
        return Err("desktop resource preflight rejected its exact package, assembly, or browser resource".to_owned());
    }
    let configured = app.path().resource_dir().ok();
    let adjacent = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf));
    let resource = configured
        .into_iter()
        .chain(adjacent)
        .find(|root| root.join("product-package/package.json").is_file())
        .ok_or_else(|| "desktop resource preflight could not locate its adjacent Product Package".to_owned())?;
    preflight_package(&resource.join("product-package"))?;
    preflight_frontend(&resource)?;
    Ok(())
}

fn next_activation_sequence(destination: &Path) -> Result<u64, String> {
    let bytes = match fs::read(destination) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(format!("activation receipt read failed: {error}")),
    };
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("activation receipt decode failed: {error}"))?;
    if value.get("artifact").and_then(Value::as_str) != Some("rusty.product.activation")
        || value.get("mainThreadCompleted").and_then(Value::as_bool) != Some(true)
    {
        return Err("existing activation receipt has an invalid identity".to_owned());
    }
    value
        .get("activationSequence")
        .and_then(Value::as_u64)
        .and_then(|sequence| sequence.checked_add(1))
        .ok_or_else(|| "existing activation receipt has no bounded activation sequence".to_owned())
}

fn write_activation_receipt(app: &AppHandle) -> Result<(), String> {
    let configured = std::env::var_os(RUSTY_PRODUCT_ACTIVATION_RECEIPT)
        .filter(|path| !path.is_empty());
    let destination = match configured.as_ref() {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err("activation receipt path must be absolute".to_owned());
            }
            path
        }
        _ => app
            .path()
            .app_local_data_dir()
            .map_err(|error| error.to_string())?
            .join(ACTIVATION_RECEIPT_FILENAME),
    };
    let root = destination
        .parent()
        .ok_or_else(|| "activation receipt path has no regular parent".to_owned())?;
    if destination.components().any(|component| {
        !matches!(component, Component::RootDir | Component::Normal(_))
    }) {
        return Err("activation receipt path contains a non-normal component".to_owned());
    }
    if configured.is_none() {
        fs::create_dir_all(root).map_err(|error| format!("activation data root: {error}"))?;
    }
    let mut current = PathBuf::from("/");
    for component in root.components() {
        if let Component::Normal(name) = component {
            current.push(name);
            let metadata = fs::symlink_metadata(&current)
                .map_err(|error| format!("activation receipt parent: {error}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("activation receipt parent contains an unmanaged path".to_owned());
            }
        }
    }
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("activation receipt destination is unmanaged".to_owned());
        }
    }
    let activation_sequence = next_activation_sequence(&destination)?;
    let stage = destination.with_extension("stage");
    if let Ok(metadata) = fs::symlink_metadata(&stage) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err("activation receipt staging path is unmanaged".to_owned());
        }
        fs::remove_file(&stage).map_err(|error| format!("activation stage cleanup: {error}"))?;
    }
    let bytes = serde_json::to_vec(&json!({
        "artifact": "rusty.product.activation",
        "mainThreadCompleted": true,
        "activationSequence": activation_sequence,
    }))
    .map_err(|error| format!("activation receipt encode: {error}"))?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stage)
        .map_err(|error| format!("activation receipt stage: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("activation receipt write: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("activation receipt sync: {error}"))?;
    fs::rename(&stage, &destination)
        .map_err(|error| format!("activation receipt publish: {error}"))
}

fn show_startup_failure(app: &AppHandle, error: &str) -> Result<(), String> {
    let window = WebviewWindowBuilder::new(
        app,
        "startup-failure",
        WebviewUrl::App("index.html".into()),
    )
    .title("Rusty Product startup failure")
    .inner_size(640.0, 360.0)
    .resizable(false)
    .build()
    .map_err(|error| error.to_string())?;
    let bounded = error.chars().take(512).collect::<String>();
    let text = serde_json::to_string(&format!("Rusty Product could not start: {bounded}"))
        .map_err(|error| error.to_string())?;
    window
        .eval(&format!(
            "document.body.textContent = {text}; document.body.style.whiteSpace = 'pre-wrap'; document.body.style.padding = '2rem';"
        ))
        .map_err(|error| error.to_string())
}

fn spawn_realtime_ticker(app: AppHandle, runtime: DesktopRuntime) {
    let stop = Arc::clone(&runtime.stop);
    let control = Arc::clone(&runtime.ticker_control);
    let gate = Arc::clone(&runtime.ticker_gate);
    let session = Arc::clone(&runtime.session);
    thread::Builder::new()
        .name("rusty-product-realtime-ticker".to_owned())
        .spawn(move || {
            let started = Instant::now();
            while !stop.load(Ordering::Acquire) {
                let gate_guard = gate.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if !ticker_is_active(&control) {
                    drop(gate_guard);
                    thread::sleep(Duration::from_millis(4));
                    continue;
                }
                let elapsed = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
                let outcome = session.advance_realtime(CanonicalU64::new(elapsed))
                    .map_err(error_text)
                    .and_then(encode_receipt)
                    .and_then(|value| {
                        if value.get("result").and_then(|result| result.get("accepted")).and_then(Value::as_bool) != Some(true) {
                            let diagnostic = value
                                .get("result")
                                .and_then(|result| result.get("diagnostic"))
                                .and_then(Value::as_str)
                                .unwrap_or("realtime advance was rejected by the runtime");
                            return Err(diagnostic.to_owned());
                        }
                        publish(&app, &value)?;
                        publish_realtime_progress(&app)
                    });
                if let Err(error) = outcome {
                    publish_terminal_failure(&app, "advance-realtime", &error);
                    deactivate_ticker(&control);
                    stop.store(true, Ordering::Release);
                    break;
                }
                drop(gate_guard);
                thread::sleep(Duration::from_millis(4));
            }
        })
        .expect("Rusty realtime ticker thread starts");
}

fn shutdown_runtime(runtime: &DesktopRuntime) {
    let _gate = runtime
        .ticker_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !runtime.stop.swap(true, Ordering::AcqRel) {
        deactivate_ticker(&runtime.ticker_control);
        let _ = runtime
            .session
            .lifecycle(ProductDevLifecycleOperation::Shutdown);
    }
}

#[tauri::command]
fn runtime_lifecycle(
    app: AppHandle,
    state: State<'_, DesktopRuntime>,
    operation: LifecycleOperation,
) -> Result<Value, String> {
    let _gate = state
        .ticker_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let shuts_down = matches!(&operation, LifecycleOperation::Shutdown);
    if state.stop.load(Ordering::Acquire) && !shuts_down {
        return Err("native realtime ticker is terminally stopped".to_owned());
    }
    let starts_realtime = matches!(&operation, LifecycleOperation::Start) && RUNTIME_MODE == "realtime";
    let activates_ticker = matches!(
        &operation,
        LifecycleOperation::Start | LifecycleOperation::Resume | LifecycleOperation::Restart
    ) && RUNTIME_MODE == "realtime";
    let deactivates_ticker = matches!(
        &operation,
        LifecycleOperation::Pause
            | LifecycleOperation::Restart
            | LifecycleOperation::ReportFault
            | LifecycleOperation::Shutdown
    ) && RUNTIME_MODE == "realtime";
    let previous_active = deactivates_ticker.then(|| deactivate_ticker(&state.ticker_control));
    let receipt = match state.session.lifecycle(operation.engine()).map_err(error_text) {
        Ok(receipt) => match encode_receipt(receipt) {
            Ok(receipt) => receipt,
            Err(error) => {
                deactivate_ticker(&state.ticker_control);
                state.stop.store(true, Ordering::Release);
                publish_terminal_failure(&app, "lifecycle", &error);
                return Err(error);
            }
        },
        Err(error) => {
            if let Some(previous_active) = previous_active {
                lock_ticker_control(&state.ticker_control).active = previous_active;
            }
            return Err(error);
        }
    };
    let accepted = receipt
        .get("result")
        .and_then(|result| result.get("accepted"))
        .and_then(Value::as_bool)
        .ok_or_else(|| "runtime lifecycle result is missing accepted".to_owned())?;
    if let Err(error) = publish(&app, &receipt) {
        // The lifecycle mutation already happened. Never restore a previous
        // ticker state against that new runtime state when its receipt cannot
        // reach the host; stop the native cadence and surface one terminal
        // failure instead.
        deactivate_ticker(&state.ticker_control);
        state.stop.store(true, Ordering::Release);
        publish_terminal_failure(&app, "lifecycle", &error);
        return Err(error);
    }
    let result = receipt
        .get("result")
        .cloned()
        .ok_or_else(|| "runtime lifecycle result is missing".to_owned())?;
    if accepted {
        if shuts_down {
            state.stop.store(true, Ordering::Release);
        }
        if activates_ticker {
            activate_ticker(&state.ticker_control);
        }
        if starts_realtime && !state.ticker_started.swap(true, Ordering::AcqRel) {
            spawn_realtime_ticker(app, state.inner().clone());
        }
    } else if let Some(previous_active) = previous_active {
        lock_ticker_control(&state.ticker_control).active = previous_active;
    }
    Ok(result)
}

#[tauri::command]
fn runtime_input(
    app: AppHandle,
    state: State<'_, DesktopRuntime>,
    batch: Value,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&batch).map_err(|error| error.to_string())?;
    publish_receipt(&app, state.session.input_json(&bytes).map_err(error_text)?)
}

#[tauri::command]
fn advance_realtime(
    app: AppHandle,
    state: State<'_, DesktopRuntime>,
    observed_time_ns: String,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&observed_time_ns).map_err(|error| error.to_string())?;
    publish_receipt(
        &app,
        state
            .session
            .advance_realtime_json(&bytes)
            .map_err(error_text)?,
    )
}

#[tauri::command]
fn admit_demand_step(app: AppHandle, state: State<'_, DesktopRuntime>) -> Result<Value, String> {
    publish_receipt(
        &app,
        state.session.admit_demand_step().map_err(error_text)?,
    )
}

#[tauri::command]
fn admit_external_step(
    app: AppHandle,
    state: State<'_, DesktopRuntime>,
    step: String,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&step).map_err(|error| error.to_string())?;
    publish_receipt(
        &app,
        state
            .session
            .admit_external_step_json(&bytes)
            .map_err(error_text)?,
    )
}

#[tauri::command]
fn complete_timeline(
    app: AppHandle,
    state: State<'_, DesktopRuntime>,
    completion: Value,
) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&completion).map_err(|error| error.to_string())?;
    publish_receipt(
        &app,
        state
            .session
            .complete_timeline_json(&bytes)
            .map_err(error_text)?,
    )
}

#[tauri::command]
fn runtime_shutdown(app: AppHandle, state: State<'_, DesktopRuntime>) -> Result<Value, String> {
    let _gate = state
        .ticker_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.stop.store(true, Ordering::Release);
    deactivate_ticker(&state.ticker_control);
    publish_receipt(
        &app,
        state
            .session
            .lifecycle(ProductDevLifecycleOperation::Shutdown)
            .map_err(error_text)?,
    )
}

fn main() {
    let runtime = GeneratedRuntime::new().expect("generated Product Runtime definition admits");
    let state = DesktopRuntime {
        session: Arc::new(ProductDevOperationOwner::new(runtime)),
        stop: Arc::new(AtomicBool::new(false)),
        ticker_started: Arc::new(AtomicBool::new(false)),
        ticker_control: Arc::new(Mutex::new(TickerControl { active: false })),
        ticker_gate: Arc::new(Mutex::new(())),
    };
    let mut builder = tauri::Builder::default();
__PLUGIN__
    builder = builder.manage(state).invoke_handler(tauri::generate_handler![
        runtime_lifecycle,
        runtime_input,
        advance_realtime,
        admit_demand_step,
        admit_external_step,
        complete_timeline,
        runtime_shutdown
    ]);
    let app = builder
        .setup(move |app| {
            if let Err(error) = preflight(app.handle()) {
                show_startup_failure(app.handle(), &error).map_err(std::io::Error::other)?;
                return Ok(());
            }
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("__TITLE__")
                .inner_size(__WIDTH__ as f64, __HEIGHT__ as f64)
                .resizable(__RESIZABLE__)
                .build()?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Rusty Tauri host builds");
    app.run(|app, event| {
            let should_shutdown = matches!(
                event,
                tauri::RunEvent::Exit
                    | tauri::RunEvent::ExitRequested { .. }
                    | tauri::RunEvent::WindowEvent {
                        event: tauri::WindowEvent::CloseRequested { .. },
                        ..
                    }
            );
            if should_shutdown {
                shutdown_runtime(&app.state::<DesktopRuntime>());
            }
        });
}
"##.to_owned();
    let frontend_hash_source = frontend_hashes
        .iter()
        .map(|(path, hash)| format!("    ({}, {}),\n", rust_string(path), rust_string(hash)))
        .collect::<String>();
    source = source.replace("__FRONTEND_TREE_SHA256__", frontend_sha256);
    source = source.replace("__FRONTEND_FILE_HASHES__", &frontend_hash_source);
    source = source.replace("__RUNTIME_MODE__", runtime_mode);
    source = source.replace("__PLUGIN__", singleton_plugin);
    source = source.replace(
        "__TITLE__",
        &wrapper.title.replace('\\', "\\\\").replace('"', "\\\""),
    );
    source = source.replace("__WIDTH__", &wrapper.width.to_string());
    source = source.replace("__HEIGHT__", &wrapper.height.to_string());
    source.replace(
        "__RESIZABLE__",
        if wrapper.resizable { "true" } else { "false" },
    )
}

fn build_tauri_release(workspace: &Path) -> Result<PathBuf, DesktopError> {
    let target = engine_checkout_root().join(DESKTOP_BUILD_TARGET);
    ensure_or_create_directory(&target, "RUSTY_DESKTOP_BUILD_CACHE")?;
    let cargo_manifest = workspace.join("Cargo.toml");
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--release")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&cargo_manifest)
        .arg("--target-dir")
        .arg(&target)
        .current_dir(workspace)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, CARGO_BUILD_TIMEOUT).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_BUILD",
            cargo_manifest.display().to_string(),
            error.to_string(),
        )
    })?;
    if !output.status.success() {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_BUILD",
            cargo_manifest.display().to_string(),
            format!(
                "Tauri release build failed: {}",
                bound_text(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                    MAX_DESKTOP_TEXT_BYTES,
                )
            ),
        ));
    }
    let binary = target.join("release").join(DESKTOP_CARGO_BINARY);
    let metadata = fs::symlink_metadata(&binary).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_BINARY",
            binary.display().to_string(),
            error.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_BINARY",
            binary.display().to_string(),
            "Tauri release build did not produce one regular product binary",
        ));
    }
    Ok(binary)
}

fn check_javascript_syntax(path: &Path) -> Result<(), DesktopError> {
    let mut command = Command::new("node");
    command
        .arg("--check")
        .arg(path)
        .current_dir(path.parent().unwrap_or_else(|| Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, BRIDGE_SYNTAX_TIMEOUT).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_BRIDGE_SYNTAX",
            path.display().to_string(),
            error.to_string(),
        )
    })?;
    if !output.status.success() {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_BRIDGE_SYNTAX",
            path.display().to_string(),
            bound_text(
                String::from_utf8_lossy(&output.stderr).into_owned(),
                MAX_DESKTOP_TEXT_BYTES,
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn make_receipt(
    root: &Path,
    manifest: &ProductManifest,
    wrapper: &SelectedWrapper,
    package_sha256: String,
    assembly_sha256: String,
    config_sha256: String,
    policy_sha256: String,
    frontend_sha256: String,
) -> Result<DesktopReceipt, DesktopError> {
    let files = collect_tree(root, "RUSTY_DESKTOP_RECEIPT_FILES")?;
    let mut entries = files
        .iter()
        .filter(|(path, _)| path.as_str() != DESKTOP_RECEIPT)
        .map(|(path, bytes)| DesktopEntry {
            path: path.clone(),
            bytes: bytes.len(),
            sha256: sha256_hex(bytes),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(DesktopReceipt {
        artifact: DESKTOP_ARTIFACT.to_owned(),
        product: manifest.product_id().to_owned(),
        wrapper_id: wrapper.id.clone(),
        wrapper_version: wrapper.version.clone(),
        application_id: wrapper.application_id.clone(),
        storage_namespace: wrapper.storage_namespace.clone(),
        package_sha256,
        assembly_sha256,
        config_sha256,
        policy_sha256,
        frontend_sha256,
        files: entries,
    })
}

fn policy_source(wrapper: &SelectedWrapper) -> Result<Vec<u8>, DesktopError> {
    let policy = DesktopPolicy {
        wrapper_id: wrapper.id.clone(),
        wrapper_version: wrapper.version.clone(),
        application_id: wrapper.application_id.clone(),
        title: wrapper.title.clone(),
        window_width: wrapper.width,
        window_height: wrapper.height,
        resizable: wrapper.resizable,
        permissions: wrapper.permissions.clone(),
        storage_namespace: wrapper.storage_namespace.clone(),
        release_channel: wrapper.release_channel.clone(),
        singleton: wrapper.singleton,
        icon_policy: "engine-fallback".to_owned(),
    };
    let mut bytes = serde_json::to_vec_pretty(&policy).map_err(|error| {
        DesktopError::new("RUSTY_DESKTOP_POLICY", DESKTOP_POLICY, error.to_string())
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn encode_receipt(receipt: &DesktopReceipt) -> Result<Vec<u8>, DesktopError> {
    let mut bytes = serde_json::to_vec_pretty(receipt).map_err(|error| {
        DesktopError::new("RUSTY_DESKTOP_RECEIPT", DESKTOP_RECEIPT, error.to_string())
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn patch_assembly_dependency_paths(assembly: &Path) -> Result<(), DesktopError> {
    let path = assembly.join("Cargo.toml");
    let text = String::from_utf8(read_regular_file(
        &path,
        "RUSTY_DESKTOP_ASSEMBLY_CARGO",
        MAX_DESKTOP_TEXT_BYTES,
    )?)
    .map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_ASSEMBLY_CARGO",
            path.display().to_string(),
            error.to_string(),
        )
    })?;
    let engine = engine_checkout_root();
    let engine_path = relative_path(assembly, &engine.join("rust/crates/rusty-engine"))?;
    let kernel_path = relative_path(assembly, &engine.join("rust/crates/product-kernel"))?;
    let mut output = String::with_capacity(text.len() + 128);
    for line in text.lines() {
        if line.trim_start().starts_with("rusty-engine = { path =") {
            output.push_str(&format!(
                "rusty-engine = {{ path = {} }}\n",
                rust_string(&engine_path)
            ));
        } else if line.trim_start().starts_with("product-kernel = { path =") {
            output.push_str(&format!(
                "product-kernel = {{ path = {} }}\n",
                rust_string(&kernel_path)
            ));
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    // The generated Assembly is itself a detached Cargo root. It is a source
    // input to this workspace, never a member of the Tauri workspace.
    replace_regular_file_atomic(&path, output.as_bytes(), "RUSTY_DESKTOP_ASSEMBLY_CARGO")?;
    patch_kernel_package_dependency_paths(assembly, &engine)
}

/// The Product Assembly package is copied into a persistent desktop build
/// workspace, so its admitted Product Kernel package closure moves with it.
/// Assembly has already made that closure explicit; this second relocation
/// only replaces the fixed Engine facade path for each copied package. Local
/// Product Kernel paths remain exactly as Assembly admitted them.
fn patch_kernel_package_dependency_paths(
    assembly: &Path,
    engine: &Path,
) -> Result<(), DesktopError> {
    let kernel = assembly.join("kernel");
    match fs::symlink_metadata(&kernel) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_KERNEL_CARGO",
                kernel.display().to_string(),
                "copied Product Kernel lane must be one regular directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(io_error(
                "RUSTY_DESKTOP_KERNEL_CARGO",
                kernel.display().to_string(),
                error,
            ));
        }
    }

    // `collect_tree` supplies the owned, bounded, no-symlink copied closure.
    // Do not walk directories independently here: Cargo manifests outside
    // that closure are not desktop build inputs.
    let files = collect_tree(&kernel, "RUSTY_DESKTOP_KERNEL_CARGO")?;
    for relative in files
        .keys()
        .filter(|relative| relative.as_str() == "Cargo.toml" || relative.ends_with("/Cargo.toml"))
    {
        patch_kernel_manifest_dependency_path(&kernel, relative, engine)?;
    }
    Ok(())
}

fn patch_kernel_manifest_dependency_path(
    kernel: &Path,
    relative: &str,
    engine: &Path,
) -> Result<(), DesktopError> {
    let path = kernel.join(relative);
    let text = String::from_utf8(read_regular_file(
        &path,
        "RUSTY_DESKTOP_KERNEL_CARGO",
        MAX_DESKTOP_TEXT_BYTES,
    )?)
    .map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_KERNEL_CARGO",
            path.display().to_string(),
            error.to_string(),
        )
    })?;
    let document: toml::Value = toml::from_str(&text).map_err(|error| {
        DesktopError::new(
            "RUSTY_DESKTOP_KERNEL_CARGO",
            path.display().to_string(),
            format!("invalid admitted Product Kernel Cargo manifest: {error}"),
        )
    })?;
    let package_directory = path.parent().ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_KERNEL_CARGO",
            path.display().to_string(),
            "copied Product Kernel Cargo manifest must have a package directory",
        )
    })?;
    let engine_dependency = document
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .and_then(|dependencies| dependencies.get("rusty-engine"))
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            DesktopError::new(
                "RUSTY_DESKTOP_KERNEL_CARGO",
                path.display().to_string(),
                "copied Product Kernel Cargo manifest must retain [dependencies.rusty-engine]",
            )
        })?;
    if engine_dependency.len() != 1
        || engine_dependency
            .get("path")
            .and_then(toml::Value::as_str)
            .is_none()
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_KERNEL_CARGO",
            path.display().to_string(),
            "[dependencies.rusty-engine] must remain the exact fixed path dependency",
        ));
    }
    let engine_path = relative_path(package_directory, &engine.join("rust/crates/rusty-engine"))?;
    let rewritten = rewrite_fixed_engine_dependency_path(&text, &engine_path).ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_KERNEL_CARGO",
            path.display().to_string(),
            "[dependencies.rusty-engine].path must remain one standalone path assignment",
        )
    })?;
    replace_regular_file_atomic(&path, rewritten.as_bytes(), "RUSTY_DESKTOP_KERNEL_CARGO")
}

fn rewrite_fixed_engine_dependency_path(source: &str, engine_path: &str) -> Option<String> {
    let mut output = String::with_capacity(source.len() + engine_path.len());
    let mut in_engine_dependency = false;
    let mut replaced = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_engine_dependency = trimmed == "[dependencies.rusty-engine]";
        }
        if in_engine_dependency && !replaced && trimmed.starts_with("path =") {
            let indentation = line.len() - line.trim_start().len();
            output.push_str(&line[..indentation]);
            output.push_str("path = ");
            output.push_str(&rust_string(engine_path));
            output.push('\n');
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    replaced.then_some(output)
}

fn engine_checkout_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("rusty-cli remains inside the Engine checkout")
        .to_path_buf()
}

fn desktop_build_workspace_path() -> PathBuf {
    engine_checkout_root().join(DESKTOP_BUILD_WORKSPACE)
}

fn replace_build_workspace(source: &Path, destination: &Path) -> Result<(), DesktopError> {
    ensure_directory(source, "RUSTY_DESKTOP_BUILD_WORKSPACE", "source workspace")?;
    let parent = destination.parent().ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_BUILD_WORKSPACE",
            destination.display().to_string(),
            "persistent build workspace must have a parent",
        )
    })?;
    ensure_or_create_directory(parent, "RUSTY_DESKTOP_BUILD_WORKSPACE")?;
    let stage = create_stage(parent, DESKTOP_BUILD_STAGE_PREFIX)?;
    if let Err(error) = copy_tree(source, &stage, "RUSTY_DESKTOP_BUILD_WORKSPACE") {
        let _ = remove_tree(&stage);
        return Err(error);
    }
    let destination_exists = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            let _ = remove_tree(&stage);
            return Err(DesktopError::new(
                "RUSTY_DESKTOP_BUILD_WORKSPACE",
                destination.display().to_string(),
                "persistent build workspace must be one regular directory",
            ));
        }
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            let _ = remove_tree(&stage);
            return Err(io_error(
                "RUSTY_DESKTOP_BUILD_WORKSPACE",
                destination.display().to_string(),
                error,
            ));
        }
    };
    let backup = if destination_exists {
        Some(fresh_path(parent, DESKTOP_BUILD_BACKUP_PREFIX)?)
    } else {
        None
    };
    let mut moved_destination = false;
    let mut moved_stage = false;
    let operation = (|| {
        if let Some(backup) = backup.as_ref() {
            fs::rename(destination, backup).map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_BUILD_WORKSPACE",
                    destination.display().to_string(),
                    error,
                )
            })?;
            moved_destination = true;
        }
        fs::rename(&stage, destination).map_err(|error| {
            io_error(
                "RUSTY_DESKTOP_BUILD_WORKSPACE",
                destination.display().to_string(),
                error,
            )
        })?;
        moved_stage = true;
        Ok::<(), DesktopError>(())
    })();
    if let Err(error) = operation {
        if moved_stage {
            let _ = remove_tree(destination);
        }
        if moved_destination {
            if let Some(backup) = backup.as_ref() {
                let _ = fs::rename(backup, destination);
            }
        }
        let _ = remove_tree(&stage);
        return Err(error);
    }
    if let Some(backup) = backup {
        remove_tree(&backup)?;
    }
    Ok(())
}

fn generated_package_name(product: &str) -> String {
    format!(
        "rusty-product-{}",
        product
            .chars()
            .map(|character| if character == '.' || character == '_' {
                '-'
            } else {
                character
            })
            .collect::<String>()
    )
}

fn release_channel_name(channel: product_model::ReleaseChannel) -> &'static str {
    match channel {
        product_model::ReleaseChannel::Development => "development",
        product_model::ReleaseChannel::Preview => "preview",
        product_model::ReleaseChannel::Stable => "stable",
    }
}

fn launcher_source() -> &'static str {
    "#!/bin/sh\nset -eu\nSELF=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nexec \"$SELF/bin/product-desktop\" \"$@\"\n"
}

fn installed_launcher_source() -> &'static str {
    "#!/bin/sh\nset -eu\nSELF=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\n: \"${RUSTY_PRODUCT_ACTIVATION_RECEIPT:=$SELF/../data/activation.json}\"\nexport RUSTY_PRODUCT_ACTIVATION_RECEIPT\nexec \"$SELF/../current/launcher.sh\" \"$@\"\n"
}

fn desktop_entry_template(wrapper: &SelectedWrapper) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=%INSTALL_ROOT%/launcher.sh\nTerminal=false\nCategories=Game;\n",
        wrapper.title
    )
}

fn desktop_exec_argument(path: &Path) -> Result<String, DesktopError> {
    let text = path.to_str().ok_or_else(|| {
        DesktopError::new(
            "RUSTY_DESKTOP_INSTALL_ENTRY",
            path.display().to_string(),
            "desktop Exec path must be valid UTF-8",
        )
    })?;
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_INSTALL_ENTRY",
            path.display().to_string(),
            "desktop Exec path must not be empty or contain control characters",
        ));
    }
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for character in text.chars() {
        match character {
            '\\' | '"' | '`' | '$' => escaped.push('\\'),
            _ => {}
        }
        escaped.push(character);
    }
    escaped.push('"');
    Ok(escaped)
}

fn package_error(error: crate::package::PackageError) -> DesktopError {
    DesktopError::new(
        "RUSTY_DESKTOP_PACKAGE",
        error.path().to_owned(),
        format!("verified Product Package rejected: {}", error.detail()),
    )
}

fn ensure_directory(path: &Path, code: &'static str, label: &str) -> Result<(), DesktopError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        DesktopError::new(
            code,
            path.display().to_string(),
            format!("{label}: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            format!("{label} must be a regular directory without symlink indirection"),
        ));
    }
    Ok(())
}

fn ensure_or_create_directory(path: &Path, code: &'static str) -> Result<(), DesktopError> {
    match fs::symlink_metadata(path) {
        Ok(_) => ensure_directory(path, code, "directory")?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir_all(path)
            .map_err(|create| io_error(code, path.display().to_string(), create))?,
        Err(error) => return Err(io_error(code, path.display().to_string(), error)),
    }
    Ok(())
}

fn create_directory(path: &Path, code: &'static str) -> Result<(), DesktopError> {
    fs::create_dir(path).map_err(|error| io_error(code, path.display().to_string(), error))
}

fn create_stage(parent: &Path, prefix: &str) -> Result<PathBuf, DesktopError> {
    for _ in 0..64 {
        let counter = STAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{prefix}{}-{counter}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(io_error(
                    "RUSTY_DESKTOP_STAGE",
                    path.display().to_string(),
                    error,
                ))
            }
        }
    }
    Err(DesktopError::new(
        "RUSTY_DESKTOP_STAGE",
        parent.display().to_string(),
        "could not allocate a bounded unique staging directory",
    ))
}

fn remove_tree(path: &Path) -> Result<(), DesktopError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(DesktopError::new(
            "RUSTY_DESKTOP_STAGE_CLEANUP",
            path.display().to_string(),
            "staging path is a symlink",
        )),
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(|error| {
            io_error(
                "RUSTY_DESKTOP_STAGE_CLEANUP",
                path.display().to_string(),
                error,
            )
        }),
        Ok(_) => Err(DesktopError::new(
            "RUSTY_DESKTOP_STAGE_CLEANUP",
            path.display().to_string(),
            "staging path is not a directory",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(
            "RUSTY_DESKTOP_STAGE_CLEANUP",
            path.display().to_string(),
            error,
        )),
    }
}

fn copy_tree(source: &Path, destination: &Path, code: &'static str) -> Result<(), DesktopError> {
    ensure_directory(source, code, "copy source")?;
    ensure_or_create_directory(destination, code)?;
    let files = collect_tree(source, code)?;
    for (relative, bytes) in files {
        write_file(&destination.join(relative), &bytes, code)?;
    }
    Ok(())
}

fn copy_tree_without_bridge(
    source: &Path,
    destination: &Path,
    code: &'static str,
) -> Result<(), DesktopError> {
    ensure_directory(source, code, "copy source")?;
    ensure_or_create_directory(destination, code)?;
    let files = collect_tree(source, code)?;
    for (relative, bytes) in files {
        if relative == "bridge.js" {
            continue;
        }
        write_file(&destination.join(relative), &bytes, code)?;
    }
    Ok(())
}

fn copy_install_release(
    source: &Path,
    destination: &Path,
    code: &'static str,
) -> Result<(), DesktopError> {
    copy_tree(source, destination, code)?;
    make_executable(&destination.join(DESKTOP_BINARY))?;
    make_executable(&destination.join(DESKTOP_LAUNCHER))
}

fn collect_tree(
    root: &Path,
    code: &'static str,
) -> Result<BTreeMap<String, Vec<u8>>, DesktopError> {
    ensure_directory(root, code, "tree root")?;
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    collect_tree_inner(root, root, &mut files, &mut total, code)?;
    Ok(files)
}

fn ensure_exact_release_tree(
    root: &Path,
    expected: &BTreeMap<String, Vec<u8>>,
) -> Result<(), DesktopError> {
    let actual = collect_tree(root, "RUSTY_DESKTOP_DRIFT_READBACK")?;
    if actual != *expected {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_DRIFT",
            root.display().to_string(),
            "existing published desktop closure differs byte-for-byte from the newly generated release; no files were changed",
        ));
    }
    Ok(())
}

fn collect_tree_inner(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    total: &mut usize,
    code: &'static str,
) -> Result<(), DesktopError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| io_error(code, directory.display().to_string(), error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(code, directory.display().to_string(), error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| io_error(code, path.display().to_string(), error))?;
        if metadata.file_type().is_symlink() {
            return Err(DesktopError::new(
                code,
                path.display().to_string(),
                "symlinks are not admitted",
            ));
        }
        if metadata.is_dir() {
            collect_tree_inner(root, &path, files, total, code)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(DesktopError::new(
                code,
                path.display().to_string(),
                "only regular files are admitted",
            ));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| {
                DesktopError::new(
                    code,
                    path.display().to_string(),
                    "tree path escaped its root",
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        if relative.is_empty() || relative.split('/').count() > 64 {
            return Err(DesktopError::new(
                code,
                relative,
                "desktop path depth is bounded",
            ));
        }
        let bytes = read_regular_file(&path, code, MAX_DESKTOP_FILE_BYTES)?;
        *total = total.checked_add(bytes.len()).ok_or_else(|| {
            DesktopError::new(code, &relative, "desktop byte accounting overflowed")
        })?;
        if *total > MAX_DESKTOP_TOTAL_BYTES {
            return Err(DesktopError::new(
                code,
                &relative,
                "desktop release exceeds its total byte bound",
            ));
        }
        if files.insert(relative.clone(), bytes).is_some() || files.len() > MAX_DESKTOP_FILES {
            return Err(DesktopError::new(
                code,
                relative,
                "desktop release has duplicate or excessive files",
            ));
        }
    }
    Ok(())
}

fn copy_regular_file(
    source: &Path,
    destination: &Path,
    code: &'static str,
) -> Result<(), DesktopError> {
    let bytes = read_regular_file(source, code, MAX_DESKTOP_FILE_BYTES)?;
    write_file(destination, &bytes, code)
}

fn read_regular_file(
    path: &Path,
    code: &'static str,
    maximum: usize,
) -> Result<Vec<u8>, DesktopError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(code, path.display().to_string(), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            "expected one regular non-symlink file",
        ));
    }
    if metadata.len() > maximum as u64 {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            format!("file exceeds {maximum} byte bound"),
        ));
    }
    fs::read(path).map_err(|error| io_error(code, path.display().to_string(), error))
}

fn write_file(path: &Path, bytes: &[u8], code: &'static str) -> Result<(), DesktopError> {
    if bytes.len() > MAX_DESKTOP_FILE_BYTES {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            "file exceeds desktop byte bound",
        ));
    }
    if let Some(parent) = path.parent() {
        ensure_or_create_directory(parent, code)?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| io_error(code, path.display().to_string(), error))?;
    file.write_all(bytes)
        .map_err(|error| io_error(code, path.display().to_string(), error))?;
    file.sync_all()
        .map_err(|error| io_error(code, path.display().to_string(), error))
}

fn replace_regular_file_atomic(
    path: &Path,
    bytes: &[u8],
    code: &'static str,
) -> Result<(), DesktopError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(code, path.display().to_string(), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            "replacement target must be a regular non-symlink file",
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        DesktopError::new(
            code,
            path.display().to_string(),
            "replacement target has no parent",
        )
    })?;
    let stage = fresh_path(parent, ".rusty-product-desktop-rewrite-")?;
    if let Err(error) = write_file(&stage, bytes, code) {
        let _ = fs::remove_file(&stage);
        return Err(error);
    }
    if let Err(error) = fs::rename(&stage, path) {
        let _ = fs::remove_file(&stage);
        return Err(io_error(code, path.display().to_string(), error));
    }
    Ok(())
}

fn make_executable(path: &Path) -> Result<(), DesktopError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| {
                io_error(
                    "RUSTY_DESKTOP_PERMISSIONS",
                    path.display().to_string(),
                    error,
                )
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|error| {
            io_error(
                "RUSTY_DESKTOP_PERMISSIONS",
                path.display().to_string(),
                error,
            )
        })?;
    }
    Ok(())
}

fn require_executable(path: &Path, code: &'static str, label: &str) -> Result<(), DesktopError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(code, path.display().to_string(), error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DesktopError::new(
            code,
            path.display().to_string(),
            format!("{label} must be one regular non-symlink file"),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(DesktopError::new(
                code,
                path.display().to_string(),
                format!("{label} is not executable"),
            ));
        }
    }
    Ok(())
}

fn relative_path(base: &Path, target: &Path) -> Result<String, DesktopError> {
    let base = fs::canonicalize(base)
        .map_err(|error| io_error("RUSTY_DESKTOP_PATH", base.display().to_string(), error))?;
    let target = fs::canonicalize(target)
        .map_err(|error| io_error("RUSTY_DESKTOP_PATH", target.display().to_string(), error))?;
    let base = base.components().collect::<Vec<_>>();
    let target = target.components().collect::<Vec<_>>();
    let common = base
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::new();
    for _ in common..base.len() {
        parts.push("..".to_owned());
    }
    for component in &target[common..] {
        parts.push(component.as_os_str().to_string_lossy().into_owned());
    }
    if parts.is_empty() {
        return Ok(".".to_owned());
    }
    Ok(parts.join("/"))
}

fn validate_install_name(value: &str, field: &str) -> Result<(), DesktopError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(DesktopError::new(
            "RUSTY_DESKTOP_INSTALL_NAME",
            field,
            "install names must be one bounded filename component",
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256_tree(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn subtree_files(files: &BTreeMap<String, Vec<u8>>, directory: &str) -> BTreeMap<String, Vec<u8>> {
    let prefix = format!("{directory}/");
    files
        .iter()
        .filter_map(|(path, bytes)| {
            path.strip_prefix(&prefix)
                .filter(|relative| !relative.is_empty())
                .map(|relative| (relative.to_owned(), bytes.clone()))
        })
        .collect()
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

fn rust_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn bound_text(mut value: String, maximum: usize) -> String {
    if value.len() <= maximum {
        return value;
    }
    value.truncate(maximum);
    while !value.is_char_boundary(value.len()) {
        value.pop();
    }
    value
}

fn io_error(code: &'static str, path: impl Into<String>, error: io::Error) -> DesktopError {
    DesktopError::new(code, path, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use product_model::{LifecycleMode, ProductManifestCandidate, WrapperCandidate};

    fn manifest(wrapper_count: usize) -> ProductManifest {
        let wrappers = (0..wrapper_count)
            .map(|index| WrapperCandidate {
                id: format!("desktop-{index}"),
                kind: WrapperKind::Tauri,
                version: "0.1.0".to_owned(),
                application_id: format!("org.rusty.desktop{index}"),
                title: format!("Desktop {index}"),
                window_width: 1280,
                window_height: 720,
                resizable: true,
                permissions: vec!["window".to_owned()],
                storage_namespace: format!("rusty.desktop{index}"),
                release_channel: product_model::ReleaseChannel::Development,
                singleton: false,
            })
            .collect();
        product_model::validate_product_manifest(ProductManifestCandidate {
            product_id: "rusty.desktop.test".to_owned(),
            composition_entrypoints: vec!["rules/main.ts".to_owned()],
            lifecycle: LifecycleMode::Realtime,
            realtime: Some(product_model::RealtimeClock::new(60, 4)),
            kernel_entry: None,
            kernel_package: None,
            ui_entry: "ui/main.ts".to_owned(),
            ui_projection_stream: None,
            ui_projection_contract: None,
            content_root: "content".to_owned(),
            compiled_composition_output: "generated/compiled-composition.json".to_owned(),
            admitted_runtime_content_output: "generated/runtime-content".to_owned(),
            product_assembly_output: "generated/product-assembly".to_owned(),
            product_bundle_output: "generated/product-bundle".to_owned(),
            wrappers,
        })
        .expect("valid test manifest")
    }

    #[test]
    fn wrapper_selection_requires_one_or_explicit_id() {
        let one = manifest(1);
        assert_eq!(select_tauri_wrapper(&one, None).unwrap().id, "desktop-0");
        let two = manifest(2);
        assert_eq!(
            select_tauri_wrapper(&two, None).unwrap_err().code(),
            "RUSTY_DESKTOP_WRAPPER_SELECTION"
        );
        assert_eq!(
            select_tauri_wrapper(&two, Some("desktop-1")).unwrap().id,
            "desktop-1"
        );
    }

    #[test]
    fn source_and_config_are_byte_deterministic() {
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        assert_eq!(
            tauri_cargo_source(&manifest, &wrapper, Path::new("/tmp")).unwrap(),
            tauri_cargo_source(&manifest, &wrapper, Path::new("/tmp")).unwrap()
        );
        assert_eq!(
            tauri_config_source(&wrapper).unwrap(),
            tauri_config_source(&wrapper).unwrap()
        );
        let config: serde_json::Value =
            serde_json::from_slice(&tauri_config_source(&wrapper).unwrap()).unwrap();
        assert_eq!(
            config["bundle"]["icon"],
            serde_json::json!(["icons/icon.png"])
        );
        assert_eq!(
            native_bridge_source(&manifest, &wrapper),
            native_bridge_source(&manifest, &wrapper)
        );
    }

    #[test]
    fn generated_bridge_passes_node_syntax_check() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        let bridge = directory.path().join("bridge.mjs");
        fs::write(&bridge, native_bridge_source(&manifest, &wrapper)).unwrap();
        check_javascript_syntax(&bridge).unwrap();
    }

    #[test]
    fn generated_bridge_admits_lifecycle_and_input_results() {
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        let source = native_bridge_source(&manifest, &wrapper);
        assert!(source.contains("returned an invalid typed result"));
        assert!(!source.contains("unwrapRuntimeReceipt"));
        assert!(source.contains("runtime_lifecycle"));
        assert!(source.contains("'lifecycle:' + operation.kind"));
        assert!(source.contains("runtime_input"));
        assert!(source.contains("}, 'input')"));
        assert!(source.contains("rusty-runtime-output"));
        assert!(source.contains("rusty-runtime-progress"));
        assert!(source.contains("rusty-runtime-terminal-failure"));
        assert!(source.contains("subscribeTerminalFailures"));
        assert!(source.contains("native runtime failure event was malformed"));
        assert!(source.contains("Promise.all([outputListenerReady, terminalFailureListenerReady])"));
    }

    #[test]
    fn generated_bridge_activates_after_listeners_and_observes_ticker_failure() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        fs::create_dir_all(directory.path().join("engine")).unwrap();
        fs::write(
            directory.path().join("engine/product-browser-host.js"),
            "export function createProductBrowserRuntimeTransport(adapter) { return adapter; }\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("bridge.mjs"),
            native_bridge_source(&manifest, &wrapper),
        )
        .unwrap();
        fs::write(
            directory.path().join("bridge-test.mjs"),
            r#"const calls = [];
const listeners = new Map();
globalThis.document = { body: { dataset: {} } };
globalThis.__TAURI__ = {
  core: { invoke: async (name, payload) => {
    calls.push(`invoke:${name}`);
    return { accepted: true, operation: payload.operation ?? name };
  } },
  event: { listen: async (name, listener) => {
    calls.push(`listen:${name}`);
    listeners.set(name, listener);
    return () => calls.push(`unlisten:${name}`);
  } },
};
const { createProductBridge } = await import('./bridge.mjs');
const bridge = createProductBridge();
const outputs = [];
const failures = [];
const unlistenFailures = bridge.transport.subscribeTerminalFailures((failure) => failures.push(failure));
const unlistenOutputs = bridge.transport.subscribeOutputs((output) => outputs.push(output));
await bridge.transport.lifecycle({ kind: 'start' });
const invoke = calls.indexOf('invoke:runtime_lifecycle');
if (invoke < 0 || calls.slice(0, invoke).filter((value) => value.startsWith('listen:')).length !== 3) {
  throw new Error(`ticker start did not wait for all native listeners: ${JSON.stringify(calls)}`);
}
listeners.get('rusty-runtime-progress')({ payload: { kind: 'runtime-progress', owner: 'rust-host' } });
listeners.get('rusty-runtime-terminal-failure')({ payload: { kind: 'runtime-failure', diagnostic: 'advance-realtime: rejected' } });
if (outputs.length !== 1 || outputs[0].kind !== 'runtime-progress' || failures.length !== 1) {
  throw new Error(`ticker event mapping failed: ${JSON.stringify({ outputs, failures })}`);
}
listeners.get('rusty-runtime-progress')({ payload: { kind: 'runtime-progress', owner: 'rust-host', extra: true } });
if (outputs.length !== 2 || outputs[1].owner !== 'invalid') {
  throw new Error(`ticker progress was not strict-decoded: ${JSON.stringify(outputs)}`);
}
unlistenOutputs();
unlistenFailures();
listeners.get('rusty-runtime-progress')({ payload: { kind: 'runtime-progress', owner: 'rust-host' } });
if (outputs.length !== 2 || !calls.includes('unlisten:rusty-runtime-output') || !calls.includes('unlisten:rusty-runtime-progress') || !calls.includes('unlisten:rusty-runtime-terminal-failure')) {
  throw new Error(`native output listener disposal was incomplete: ${JSON.stringify(calls)}`);
}
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg(directory.path().join("bridge-test.mjs"))
            .current_dir(directory.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn desktop_bootstrap_reports_visible_startup_failures() {
        let source = desktop_bootstrap_source();
        assert!(source.contains("import('./engine/product-browser-host.js')"));
        assert!(source.contains("data-desktop-startup-error"));
        assert!(source.contains("data-startup-error"));
        assert!(source.contains("MAX_STARTUP_ERROR_BYTES"));
    }

    #[test]
    fn generated_bootstrap_passes_node_syntax_check() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = directory.path().join("main.mjs");
        fs::write(&bootstrap, desktop_bootstrap_source()).unwrap();
        check_javascript_syntax(&bootstrap).unwrap();
    }

    #[test]
    fn generated_host_keeps_native_receipt_and_shutdown_guards() {
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        let source = tauri_main_source(&manifest, &wrapper);
        assert!(source.contains("preflight_package"));
        assert!(source.contains("sha256_hex"));
        assert!(source.contains("RUSTY_PRODUCT_ACTIVATION_RECEIPT"));
        assert!(source.contains("mainThreadCompleted"));
        assert!(source.contains("CloseRequested"));
        assert!(source.contains("shutdown_runtime"));
        assert!(source.contains("show_startup_failure"));
        assert!(source.contains("publish_terminal_failure"));
        assert!(source.contains("publish_realtime_progress"));
        assert!(source.contains("rusty-runtime-terminal-failure"));
        assert!(source.contains("rusty-runtime-progress"));
        assert!(source.contains("bounded_terminal_diagnostic"));
        assert!(source.contains("realtime advance was rejected by the runtime"));
        assert!(source.contains(".map_err(error_text)"));
        assert!(source.contains(".and_then(encode_receipt)"));
        assert!(source.contains("publish(&app, &value)"));
        assert!(source.contains("stop.store(true, Ordering::Release)"));
        assert!(source
            .contains("let starts_realtime = matches!(&operation, LifecycleOperation::Start)"));
        assert!(source.contains("state.ticker_started.swap(true, Ordering::AcqRel)"));
        assert!(source.contains("ticker_control: Arc<Mutex<TickerControl>>"));
        assert!(source.contains("ticker_gate: Arc<Mutex<()>>"));
        assert!(source.contains("let activates_ticker = matches!("));
        assert!(source.contains("let deactivates_ticker = matches!("));
        assert!(source.contains("deactivate_ticker(&state.ticker_control)"));
        assert!(source.contains("activate_ticker(&state.ticker_control)"));
        assert!(source.contains("let gate_guard = gate.lock()"));
        assert!(source.contains("let previous_active = deactivates_ticker.then"));
        assert!(source.contains("if state.stop.load(Ordering::Acquire) && !shuts_down"));
        assert!(source.contains("native realtime ticker is terminally stopped"));
        assert!(!source.contains("if RUNTIME_MODE == \"realtime\" {"));
        let cargo = tauri_cargo_source(&manifest, &wrapper, Path::new("/tmp")).unwrap();
        assert!(cargo.contains("name = \"rusty-product-desktop\""));
        assert!(cargo.contains("tauri = { version = \"2.11.5\""));
        assert!(cargo.contains("tauri-build = \"2.6.3\""));
        assert!(cargo.contains("sha2 = \"0.10\""));
        assert!(cargo.contains("exclude = [\"product-assembly\"]"));
    }

    #[test]
    fn wrapper_policy_retains_all_selected_fields() {
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        let bytes = policy_source(&wrapper).unwrap();
        let policy: DesktopPolicy = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(policy.wrapper_id, "desktop-0");
        assert_eq!(policy.application_id, "org.rusty.desktop0");
        assert_eq!(policy.permissions, vec!["window"]);
        assert_eq!(policy.storage_namespace, "rusty.desktop0");
        assert_eq!(policy.release_channel, "development");
        assert!(!policy.singleton);
        assert_eq!(policy.icon_policy, "engine-fallback");
    }

    #[test]
    fn assembly_dependency_patch_replaces_existing_scratch_file() {
        let directory = tempfile::tempdir().unwrap();
        let assembly = directory.path().join("product-assembly");
        fs::create_dir_all(&assembly).unwrap();
        fs::write(
            assembly.join("Cargo.toml"),
            "rusty-engine = { path = \"stale-engine\" }\nproduct-kernel = { path = \"stale-kernel\" }\n",
        )
        .unwrap();
        patch_assembly_dependency_paths(&assembly).unwrap();
        let patched = fs::read_to_string(assembly.join("Cargo.toml")).unwrap();
        assert!(!patched.contains("stale-engine"));
        assert!(!patched.contains("stale-kernel"));
        assert!(patched.contains("rusty-engine = { path = \""));
        assert!(patched.contains("product-kernel = { path = \""));
    }

    #[test]
    fn assembly_dependency_patch_relocates_nested_kernel_engine_paths_without_touching_local_paths()
    {
        let directory = tempfile::tempdir().unwrap();
        let assembly = directory.path().join("product-assembly");
        let kernel = assembly.join("kernel");
        let local = kernel.join("runtime");
        fs::create_dir_all(&local).unwrap();
        fs::write(
            assembly.join("Cargo.toml"),
            "[dependencies]\nrusty-engine = { path = \"stale-engine\" }\nproduct-kernel = { path = \"stale-kernel\" }\n",
        )
        .unwrap();
        fs::write(
            kernel.join("Cargo.toml"),
            "[package]\nname = \"kernel-root\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies.runtime]\npath = \"runtime\"\n\n[dependencies.rusty-engine]\npath = \"../../../../rusty-engine/rust/crates/rusty-engine\"\n",
        )
        .unwrap();
        fs::write(
            local.join("Cargo.toml"),
            "[package]\nname = \"kernel-runtime\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies.rusty-engine]\npath = \"../../../../../rusty-engine/rust/crates/rusty-engine\"\n",
        )
        .unwrap();

        patch_assembly_dependency_paths(&assembly).unwrap();

        let root: toml::Value =
            toml::from_str(&fs::read_to_string(kernel.join("Cargo.toml")).unwrap()).unwrap();
        let root_dependencies = root
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .unwrap();
        assert_eq!(
            root_dependencies
                .get("runtime")
                .and_then(toml::Value::as_table)
                .and_then(|value| value.get("path"))
                .and_then(toml::Value::as_str),
            Some("runtime")
        );
        let engine_root = engine_checkout_root().join("rust/crates/rusty-engine");
        assert_eq!(
            root_dependencies
                .get("rusty-engine")
                .and_then(toml::Value::as_table)
                .and_then(|value| value.get("path"))
                .and_then(toml::Value::as_str),
            Some(relative_path(&kernel, &engine_root).unwrap().as_str())
        );

        let nested: toml::Value =
            toml::from_str(&fs::read_to_string(local.join("Cargo.toml")).unwrap()).unwrap();
        assert_eq!(
            nested
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .and_then(|dependencies| dependencies.get("rusty-engine"))
                .and_then(toml::Value::as_table)
                .and_then(|value| value.get("path"))
                .and_then(toml::Value::as_str),
            Some(relative_path(&local, &engine_root).unwrap().as_str())
        );
    }

    #[test]
    fn persistent_build_workspace_replacement_is_exact_and_repeatable() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let destination = directory.path().join("target/product-desktop-workspace");
        fs::create_dir_all(source.join("frontend")).unwrap();
        fs::write(source.join("frontend/main.js"), b"first").unwrap();
        replace_build_workspace(&source, &destination).unwrap();
        assert_eq!(
            fs::read(destination.join("frontend/main.js")).unwrap(),
            b"first"
        );
        fs::write(destination.join("stale"), b"remove").unwrap();
        fs::write(source.join("frontend/main.js"), b"second").unwrap();
        replace_build_workspace(&source, &destination).unwrap();
        assert_eq!(
            fs::read(destination.join("frontend/main.js")).unwrap(),
            b"second"
        );
        assert!(!destination.join("stale").exists());
        replace_build_workspace(&source, &destination).unwrap();
        assert_eq!(
            fs::read(destination.join("frontend/main.js")).unwrap(),
            b"second"
        );
    }

    #[test]
    fn scratch_source_closure_keeps_exact_product_bundle_separate_from_frontend() {
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("bundle");
        let workspace = directory.path().join("workspace");
        fs::create_dir_all(bundle.join("artifacts")).unwrap();
        fs::write(bundle.join("artifacts/compiled.json"), b"admitted").unwrap();
        fs::write(bundle.join("bridge.js"), b"browser bridge").unwrap();
        copy_tree(&bundle, &workspace.join("product-bundle"), "test").unwrap();
        copy_tree_without_bridge(&bundle, &workspace.join("frontend"), "test").unwrap();
        assert_eq!(
            fs::read(workspace.join("product-bundle/artifacts/compiled.json")).unwrap(),
            b"admitted"
        );
        assert!(!workspace.join("frontend/bridge.js").exists());
    }

    fn fake_release(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join("frontend")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("product-package/closure/runtime")).unwrap();
        fs::write(root.join(DESKTOP_BINARY), b"binary").unwrap();
        make_executable(&root.join(DESKTOP_BINARY)).unwrap();
        fs::write(root.join("frontend/index.html"), b"index").unwrap();
        fs::write(root.join("product-package/package.json"), b"{}\n").unwrap();
        fs::write(root.join(DESKTOP_CONFIG), b"{}\n").unwrap();
        fs::write(
            root.join(DESKTOP_POLICY),
            serde_json::to_vec(&DesktopPolicy {
                wrapper_id: "desktop-0".to_owned(),
                wrapper_version: "0.1.0".to_owned(),
                application_id: "org.rusty.desktop0".to_owned(),
                title: "Desktop 0".to_owned(),
                window_width: 1280,
                window_height: 720,
                resizable: true,
                permissions: vec!["window".to_owned()],
                storage_namespace: "rusty.desktop0".to_owned(),
                release_channel: "development".to_owned(),
                singleton: false,
                icon_policy: "engine-fallback".to_owned(),
            })
            .unwrap(),
        )
        .unwrap();
        fs::write(root.join(DESKTOP_LAUNCHER), launcher_source()).unwrap();
        make_executable(&root.join(DESKTOP_LAUNCHER)).unwrap();
        fs::write(
            root.join(DESKTOP_ENTRY_TEMPLATE),
            b"Exec=%INSTALL_ROOT%/launcher.sh\n",
        )
        .unwrap();
        let files = collect_tree(root, "test").unwrap();
        let receipt = DesktopReceipt {
            artifact: DESKTOP_ARTIFACT.to_owned(),
            product: "rusty.desktop.test".to_owned(),
            wrapper_id: "desktop-0".to_owned(),
            wrapper_version: "0.1.0".to_owned(),
            application_id: "org.rusty.desktop0".to_owned(),
            storage_namespace: "rusty.desktop0".to_owned(),
            package_sha256: "0".repeat(64),
            assembly_sha256: "1".repeat(64),
            config_sha256: sha256_hex(b"{}\n"),
            policy_sha256: sha256_hex(&fs::read(root.join(DESKTOP_POLICY)).unwrap()),
            frontend_sha256: sha256_tree(&subtree_files(&files, DESKTOP_FRONTEND)),
            files: files
                .iter()
                .map(|(path, bytes)| DesktopEntry {
                    path: path.clone(),
                    bytes: bytes.len(),
                    sha256: sha256_hex(bytes),
                })
                .collect(),
        };
        fs::write(
            root.join(DESKTOP_RECEIPT),
            encode_receipt(&receipt).unwrap(),
        )
        .unwrap();
        root.to_path_buf()
    }

    #[test]
    fn relocated_readback_rejects_drift() {
        let directory = tempfile::tempdir().unwrap();
        let release = fake_release(&directory.path().join("release"));
        let error = verify_relocated(&release).unwrap_err();
        assert_eq!(error.code(), "RUSTY_DESKTOP_PACKAGE");
        fs::write(release.join("frontend/index.html"), b"drift").unwrap();
        assert_eq!(
            verify_relocated(&release).unwrap_err().code(),
            "RUSTY_DESKTOP_FILE_DRIFT"
        );
    }

    #[test]
    fn idempotency_rejects_changed_binary_with_same_policy_and_package() {
        let directory = tempfile::tempdir().unwrap();
        let release = fake_release(&directory.path().join("release"));
        let expected = collect_tree(&release, "test").unwrap();
        fs::write(release.join(DESKTOP_BINARY), b"new binary").unwrap();
        make_executable(&release.join(DESKTOP_BINARY)).unwrap();
        assert_eq!(
            ensure_exact_release_tree(&release, &expected)
                .unwrap_err()
                .code(),
            "RUSTY_DESKTOP_DRIFT"
        );
    }

    #[cfg(unix)]
    #[test]
    fn relocated_readback_rejects_non_executable_binary() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let release = fake_release(&directory.path().join("release"));
        let mut permissions = fs::metadata(release.join(DESKTOP_BINARY))
            .unwrap()
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(release.join(DESKTOP_BINARY), permissions).unwrap();
        assert_eq!(
            verify_relocated(&release).unwrap_err().code(),
            "RUSTY_DESKTOP_BINARY"
        );
    }

    #[cfg(unix)]
    #[test]
    fn relocated_readback_rejects_non_executable_launcher() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let release = fake_release(&directory.path().join("release"));
        let mut permissions = fs::metadata(release.join(DESKTOP_LAUNCHER))
            .unwrap()
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(release.join(DESKTOP_LAUNCHER), permissions).unwrap();
        assert_eq!(
            verify_relocated(&release).unwrap_err().code(),
            "RUSTY_DESKTOP_LAUNCHER"
        );
    }

    #[test]
    fn desktop_exec_argument_quotes_spaces_and_metacharacters() {
        assert_eq!(
            desktop_exec_argument(Path::new("/tmp/Rusty Product/$current/launcher.sh")).unwrap(),
            "\"/tmp/Rusty Product/\\$current/launcher.sh\""
        );
    }

    #[test]
    fn canonical_package_identity_rejects_metadata_tampering() {
        let manifest = manifest(1);
        let wrapper = select_tauri_wrapper(&manifest, None).unwrap();
        let policy = CanonicalWrapperPolicy {
            id: wrapper.id.clone(),
            kind: "tauri".to_owned(),
            version: wrapper.version.clone(),
            application_id: wrapper.application_id.clone(),
            title: wrapper.title.clone(),
            window_width: wrapper.width,
            window_height: wrapper.height,
            resizable: wrapper.resizable,
            permissions: wrapper.permissions.clone(),
            storage_namespace: wrapper.storage_namespace.clone(),
            release_channel: wrapper.release_channel.clone(),
            singleton: wrapper.singleton,
        };
        let package = CanonicalPackageReceipt {
            artifact: "rusty.product.package".to_owned(),
            product: manifest.product_id().to_owned(),
            assembly_sha256: "a".repeat(64),
            entries: vec![CanonicalPackageEntry {
                path: "closure/rusty.toml".to_owned(),
                bytes: 1,
                sha256: "b".repeat(64),
            }],
            wrapper_policy: vec![policy],
        };
        validate_canonical_package_identity(&package, &manifest, &wrapper, &"a".repeat(64))
            .unwrap();
        let mut forged_product = package.clone();
        forged_product.product = "rusty.forged".to_owned();
        assert_eq!(
            validate_canonical_package_identity(
                &forged_product,
                &manifest,
                &wrapper,
                &"a".repeat(64),
            )
            .unwrap_err()
            .code(),
            "RUSTY_DESKTOP_PACKAGE_IDENTITY"
        );
        let mut forged_policy = package;
        forged_policy.wrapper_policy[0].application_id = "org.rusty.forged".to_owned();
        assert_eq!(
            validate_canonical_package_identity(
                &forged_policy,
                &manifest,
                &wrapper,
                &"a".repeat(64),
            )
            .unwrap_err()
            .code(),
            "RUSTY_DESKTOP_PACKAGE_POLICY"
        );
    }

    #[test]
    fn late_sidecar_failure_rolls_back_the_complete_install_transaction() {
        let root = tempfile::tempdir().unwrap();
        let releases = root.path().join("releases");
        fs::create_dir(&releases).unwrap();
        let release = releases.join("new-release");
        let staged_release = releases.join(".staged-release");
        fs::create_dir(&staged_release).unwrap();
        fs::write(staged_release.join("version"), b"new-release").unwrap();
        let current = root.path().join("current");
        let previous = root.path().join("previous");
        let staged_current = root.path().join(".staged-current");
        for (path, value) in [
            (&current, b"current".as_slice()),
            (&previous, b"previous".as_slice()),
            (&staged_current, b"new-current".as_slice()),
        ] {
            fs::create_dir(path).unwrap();
            fs::write(path.join("version"), value).unwrap();
        }
        let launcher = root.path().join("bin/launcher");
        let staged_launcher = root.path().join("bin/.staged-launcher");
        let entry = root.path().join("share/applications/product.desktop");
        let staged_entry = root.path().join("share/applications/.staged-entry");
        fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::write(&staged_launcher, b"launcher").unwrap();
        fs::write(&staged_entry, b"entry").unwrap();
        fs::write(&entry, b"concurrent-conflict").unwrap();

        let error = commit_install_transaction(
            &release,
            Some(&staged_release),
            &current,
            &previous,
            Some(&staged_current),
            true,
            true,
            &launcher,
            Some(&staged_launcher),
            &entry,
            Some(&staged_entry),
        )
        .unwrap_err();
        assert_eq!(error.code(), "RUSTY_DESKTOP_INSTALL_ENTRY");
        assert_eq!(fs::read(current.join("version")).unwrap(), b"current");
        assert_eq!(fs::read(previous.join("version")).unwrap(), b"previous");
        assert_eq!(fs::read(&entry).unwrap(), b"concurrent-conflict");
        assert!(!launcher.exists());
        assert!(!release.exists());
    }

    #[cfg(unix)]
    #[test]
    fn install_release_copy_restores_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        let destination = directory.path().join("destination");
        fs::create_dir_all(source.join("bin")).unwrap();
        fs::write(source.join(DESKTOP_BINARY), b"binary").unwrap();
        fs::write(source.join(DESKTOP_LAUNCHER), b"#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(source.join(DESKTOP_BINARY))
            .unwrap()
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(source.join(DESKTOP_BINARY), permissions).unwrap();
        copy_install_release(&source, &destination, "test").unwrap();
        for executable in [DESKTOP_BINARY, DESKTOP_LAUNCHER] {
            assert_ne!(
                fs::metadata(destination.join(executable))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0
            );
        }
    }

    #[test]
    fn unmanaged_install_is_refused_without_touching_data() {
        let package_dir = tempfile::tempdir().unwrap();
        let release = fake_release(&package_dir.path().join("release"));
        let user = tempfile::tempdir().unwrap();
        fs::create_dir_all(user.path().join("current")).unwrap();
        fs::write(user.path().join("current/unmanaged"), b"keep").unwrap();
        fs::create_dir_all(user.path().join("data")).unwrap();
        fs::write(user.path().join("data/save"), b"save").unwrap();
        let error = install(&release, user.path(), &InstallOptions::default()).unwrap_err();
        assert_eq!(error.code(), "RUSTY_DESKTOP_INSTALL_UNMANAGED");
        assert_eq!(fs::read(user.path().join("data/save")).unwrap(), b"save");
        assert_eq!(
            fs::read(user.path().join("current/unmanaged")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn checked_install_rotation_preserves_previous_release() {
        let user = tempfile::tempdir().unwrap();
        let current = user.path().join("current");
        let previous = user.path().join("previous");
        let staged = user.path().join(".stage");
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&previous).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::write(current.join("version"), b"newer").unwrap();
        fs::write(previous.join("version"), b"older").unwrap();
        fs::write(staged.join("version"), b"latest").unwrap();
        rotate_install_releases(&current, &previous, &staged, true).unwrap();
        assert_eq!(fs::read(current.join("version")).unwrap(), b"latest");
        assert_eq!(fs::read(previous.join("version")).unwrap(), b"newer");
        assert!(!staged.exists());
    }
}
