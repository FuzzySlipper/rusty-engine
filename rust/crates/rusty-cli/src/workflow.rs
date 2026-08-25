//! Executable Product Model workflow orchestration.
//!
//! This is deliberately a small command-side composition owner.  It discovers
//! an explicit product, converts only the Engine-owned authoring/browser
//! artifacts to immutable materializer inputs, optionally asks the legacy
//! Product Kernel contract for its capabilities, then plans or verifies the
//! exact generated assembly. Product code never reaches back into this checkout
//! at runtime: the only paths rendered into an assembly are product-relative
//! Cargo dependencies.

use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use product_assembly::{
    plan_product_assembly_with_kernel_capabilities,
    verify_product_assembly_with_kernel_capabilities, AssemblyGenerationInputs, AssemblyPlan,
    AssemblyReceipt,
};
use product_materializer::{
    materialize_product, EngineAsset, EngineAssets, MaterializationLimits, MaterializationToolchain,
};
use product_model::{
    decode_product_manifest, ProductKernelCapabilityDescriptor, ProductManifest,
    MAX_PRODUCT_MANIFEST_BYTES,
};
use serde::Deserialize;

use crate::{
    kernel_probe::{probe_capabilities, run_bounded, KernelProbeError},
    report::Diagnostic,
};

const MANIFEST_NAME: &str = "rusty.toml";
const MAX_DISCOVERY_ANCESTORS: usize = 64;
const MAX_ENGINE_ARTIFACT_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ENGINE_ARTIFACT_ENTRIES: usize = 1_024;
const CARGO_TIMEOUT: Duration = Duration::from_secs(120);

/// The Cargo profile selected for a generated Product Assembly executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildProfile {
    Debug,
    Release,
}

impl BuildProfile {
    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }

    fn cargo_argument(self) -> Option<&'static str> {
        match self {
            Self::Debug => None,
            Self::Release => Some("--release"),
        }
    }
}

/// A complete in-memory admission of an explicit product root.  Constructing
/// this value does not write below the product root; callers choose publication
/// separately so `check` and inspection routes remain read-only.
#[derive(Debug)]
pub(crate) struct AdmittedProduct {
    root: PathBuf,
    manifest: ProductManifest,
    kernel_capabilities: Vec<ProductKernelCapabilityDescriptor>,
    inputs: AssemblyGenerationInputs,
    plan: AssemblyPlan,
}

impl AdmittedProduct {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn manifest(&self) -> &ProductManifest {
        &self.manifest
    }

    pub(crate) fn kernel_capabilities(&self) -> &[ProductKernelCapabilityDescriptor] {
        &self.kernel_capabilities
    }

    pub(crate) fn compiled_composition(&self) -> &[u8] {
        self.inputs.compiled_composition()
    }

    pub(crate) fn assembly_root(&self) -> PathBuf {
        self.root
            .join(self.manifest.product_assembly_output().as_str())
    }
}

/// A published and read-back exact Product Assembly.
#[derive(Debug)]
pub(crate) struct PreparedProduct {
    admitted: AdmittedProduct,
    receipt: AssemblyReceipt,
}

impl PreparedProduct {
    pub(crate) fn root(&self) -> &Path {
        self.admitted.root()
    }

    pub(crate) fn manifest(&self) -> &ProductManifest {
        self.admitted.manifest()
    }

    pub(crate) fn receipt(&self) -> &AssemblyReceipt {
        &self.receipt
    }

    pub(crate) fn kernel_capabilities(&self) -> &[ProductKernelCapabilityDescriptor] {
        self.admitted.kernel_capabilities()
    }
}

/// Exact generated executable resolved after a successful Cargo build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedBinary {
    profile: BuildProfile,
    path: PathBuf,
}

impl GeneratedBinary {
    pub(crate) const fn profile(&self) -> BuildProfile {
        self.profile
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Discovers the product root from the explicitly supplied directory or file,
/// materializes its declared authoring/UI lanes in a temporary closure, probes
/// an optional Product Kernel through compiled Rust, and plans its exact
/// Product Assembly without touching `generated/`.
pub(crate) fn admit_product(start: impl AsRef<Path>) -> Result<AdmittedProduct, Diagnostic> {
    let root = discover_product_root(start.as_ref())?;
    let manifest = read_manifest(&root)?;
    let engine = current_engine_checkout()?;
    let assets = collect_engine_assets(&engine)?;
    let toolchain = materialization_toolchain(&engine)?;
    let materialized = materialize_product(
        &root,
        &manifest,
        &assets,
        &toolchain,
        MaterializationLimits::default(),
    )
    .map_err(materializer_diagnostic)?;

    let kernel_capabilities = if manifest.has_kernel() {
        probe_capabilities(
            &root,
            manifest.kernel_entry(),
            manifest.kernel_package(),
            &engine.facade,
        )
        .map_err(probe_diagnostic)?
    } else {
        Vec::new()
    };
    let assembly_root = root.join(manifest.product_assembly_output().as_str());
    let engine_path = cargo_relative_path(&assembly_root, &engine.facade)?;
    let mut inputs = materialized
        .assembly_inputs()
        .map_err(assembly_diagnostic)?
        .with_engine_dependency_path(engine_path)
        .map_err(assembly_diagnostic)?;
    if manifest.has_kernel() {
        let kernel_path = cargo_relative_path(&assembly_root, &engine.product_kernel)?;
        inputs = inputs
            .with_kernel_dependency_path(kernel_path)
            .map_err(assembly_diagnostic)?;
    }
    let plan = plan_product_assembly_with_kernel_capabilities(
        &root,
        &manifest,
        &inputs,
        &kernel_capabilities,
    )
    .map_err(assembly_diagnostic)?;
    Ok(AdmittedProduct {
        root,
        manifest,
        kernel_capabilities,
        inputs,
        plan,
    })
}

/// Verifies an already-published generated closure against a fresh in-memory
/// admission.  `None` means no assembly receipt exists yet; it is not an
/// implicit generation request.
pub(crate) fn verify_generated_product(
    admitted: &AdmittedProduct,
) -> Result<Option<AssemblyReceipt>, Diagnostic> {
    let receipt = admitted.assembly_root().join("assembly.json");
    match fs::symlink_metadata(&receipt) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(workflow_error(
            "RUSTY_GENERATED_READ",
            receipt.display().to_string(),
            format!(
                "Product Assembly owner could not inspect the generated receipt: {error}. Remedy: repair the generated lane or run `rusty build` to republish it."
            ),
        )),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(workflow_error(
                "RUSTY_GENERATED_RECEIPT_KIND",
                "generated/product-assembly/assembly.json",
                "Product Assembly owner requires a regular non-symlink receipt. Remedy: remove the invalid generated output and rerun `rusty build`.",
            ))
        }
        Ok(_) => verify_product_assembly_with_kernel_capabilities(
            admitted.root(),
            admitted.manifest(),
            &admitted.inputs,
            admitted.kernel_capabilities(),
        )
        .map(Some)
        .map_err(assembly_diagnostic),
    }
}

/// Publishes an admitted Product Assembly as one Engine-owned generated tree,
/// then read-backs the result against the fresh plan.  It is safe to call
/// repeatedly: each call replaces the complete generated tree atomically.
pub(crate) fn publish_admitted_product(
    admitted: AdmittedProduct,
) -> Result<PreparedProduct, Diagnostic> {
    admitted
        .plan
        .publish(&admitted.root)
        .map_err(assembly_diagnostic)?;
    let receipt = verify_product_assembly_with_kernel_capabilities(
        &admitted.root,
        &admitted.manifest,
        &admitted.inputs,
        &admitted.kernel_capabilities,
    )
    .map_err(assembly_diagnostic)?;
    Ok(PreparedProduct { admitted, receipt })
}

/// Freshly admits and publishes a product.  Development and build commands
/// use this rather than reading stale generated source as an input.
pub(crate) fn prepare_product(start: impl AsRef<Path>) -> Result<PreparedProduct, Diagnostic> {
    publish_admitted_product(admit_product(start)?)
}

/// Freshly publishes the exact assembly and builds its detached generated
/// Cargo package.  The returned path is derived from Cargo metadata and then
/// checked as a regular executable file; no guessed product-name path leaks
/// into the command contract.
pub(crate) fn build_product(
    start: impl AsRef<Path>,
    profile: BuildProfile,
) -> Result<GeneratedBinary, Diagnostic> {
    let prepared = prepare_product(start)?;
    build_prepared_product(&prepared, profile)
}

pub(crate) fn build_prepared_product(
    prepared: &PreparedProduct,
    profile: BuildProfile,
) -> Result<GeneratedBinary, Diagnostic> {
    let build_source = stage_build_source(prepared)?;
    let assembly_root = build_source.path().to_path_buf();
    let cargo_manifest = assembly_root.join("Cargo.toml");
    let metadata = cargo_metadata(&cargo_manifest)?;
    let target = metadata.target_for(&cargo_manifest)?;
    // Generated source is isolated and removed after the build, while Cargo's
    // ordinary Engine checkout cache is reused. The release package copies the
    // resulting binary by bytes, so no runtime path authority is retained.
    let target_directory = current_engine_checkout()?.root.join("target");

    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&cargo_manifest)
        .arg("--target-dir")
        .arg(&target_directory);
    if let Some(argument) = profile.cargo_argument() {
        command.arg(argument);
    }
    command
        .current_dir(&assembly_root)
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, CARGO_TIMEOUT).map_err(process_diagnostic)?;
    if !output.status.success() {
        return Err(workflow_error(
            "RUSTY_GENERATED_BUILD",
            "generated/product-assembly/Cargo.toml",
            format!(
                "Generated Product Assembly owner could not build the exact admitted runtime closure. Source: Cargo diagnostics: {}. Remedy: fix the declared Product Kernel or Product Model inputs, then rerun `rusty build`.",
                output_text(&output.stderr)
            ),
        ));
    }

    let mut binary = target_directory
        .join(profile.directory())
        .join(&target.name);
    if cfg!(windows) {
        binary.set_extension("exe");
    }
    let binary_metadata = fs::symlink_metadata(&binary).map_err(|error| {
        workflow_error(
            "RUSTY_GENERATED_BINARY",
            binary.display().to_string(),
            format!(
                "Generated Product Assembly owner did not produce Cargo's declared binary: {error}. Remedy: inspect the generated Cargo target and rerun `rusty build`."
            ),
        )
    })?;
    if binary_metadata.file_type().is_symlink() || !binary_metadata.is_file() {
        return Err(workflow_error(
            "RUSTY_GENERATED_BINARY",
            binary.display().to_string(),
            "Generated Product Assembly owner produced an invalid binary path. Remedy: clean only this product's generated target and rerun `rusty build`.",
        ));
    }
    build_source.close().map_err(|error| {
        workflow_error(
            "RUSTY_GENERATED_BUILD_CLEANUP",
            "generated",
            format!(
                "Generated Product Assembly owner built successfully but could not remove its staged source closure: {error}. Remedy: remove only the reported generated stage after preserving diagnostics."
            ),
        )
    })?;
    Ok(GeneratedBinary {
        profile,
        path: binary,
    })
}

fn stage_build_source(prepared: &PreparedProduct) -> Result<tempfile::TempDir, Diagnostic> {
    let generated = prepared.root().join("generated");
    let stage = tempfile::Builder::new()
        .prefix(".rusty-product-build-source-")
        .tempdir_in(&generated)
        .map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                "generated",
                format!(
                    "Generated Product Assembly owner could not create its isolated build source: {error}. Remedy: restore a writable regular generated directory."
                ),
            )
        })?;
    let prefix = format!(
        "{}/",
        prepared.manifest().product_assembly_output().as_str()
    );
    for entry in prepared.receipt().entries() {
        let Some(relative) = entry.path().strip_prefix(&prefix) else {
            continue;
        };
        let source = prepared.root().join(entry.path());
        let metadata = fs::symlink_metadata(&source).map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                entry.path(),
                format!(
                    "Generated Product Assembly owner could not read an admitted build input: {error}. Remedy: rerun `rusty build` to republish the exact closure."
                ),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                entry.path(),
                "Generated Product Assembly owner requires every staged input to remain a regular non-symlink file. Remedy: republish the exact assembly.",
            ));
        }
        let bytes = fs::read(&source).map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                entry.path(),
                format!("could not read admitted build input: {error}. Remedy: republish the exact assembly."),
            )
        })?;
        if bytes.len() != entry.byte_length() || sha256_hex(&bytes) != entry.sha256() {
            return Err(workflow_error(
                "RUSTY_GENERATED_BUILD_DRIFT",
                entry.path(),
                "Generated Product Assembly input changed after verification. Remedy: rerun `rusty build` after concurrent publication completes.",
            ));
        }
        let destination = stage.path().join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                workflow_error(
                    "RUSTY_GENERATED_BUILD_STAGE",
                    relative,
                    format!("could not create staged build directories: {error}. Remedy: repair generated directory permissions."),
                )
            })?;
        }
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&destination).map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                relative,
                format!("could not create staged build input: {error}. Remedy: retry after any concurrent build completes."),
            )
        })?;
        use std::io::Write;
        file.write_all(&bytes).map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_BUILD_STAGE",
                relative,
                format!("could not write staged build input: {error}. Remedy: repair generated directory capacity and permissions."),
            )
        })?;
    }
    if !stage.path().join("Cargo.toml").is_file() {
        return Err(workflow_error(
            "RUSTY_GENERATED_BUILD_STAGE",
            "generated/product-assembly/Cargo.toml",
            "Product Assembly receipt did not supply its generated Cargo manifest. Remedy: republish the exact assembly.",
        ));
    }
    Ok(stage)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut result = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn discover_product_root(start: &Path) -> Result<PathBuf, Diagnostic> {
    let metadata = fs::metadata(start).map_err(|error| {
        workflow_error(
            "RUSTY_PRODUCT_DISCOVERY_START",
            start.display().to_string(),
            format!(
                "Product Model owner cannot inspect the explicit product path: {error}. Remedy: pass --path to a product directory or a file below one."
            ),
        )
    })?;
    let start = if metadata.is_file() {
        start.parent().ok_or_else(|| {
            workflow_error(
                "RUSTY_PRODUCT_DISCOVERY_START",
                start.display().to_string(),
                "Product Model owner requires a file path with a parent directory. Remedy: pass a product directory or an in-product file.",
            )
        })?
    } else if metadata.is_dir() {
        start
    } else {
        return Err(workflow_error(
            "RUSTY_PRODUCT_DISCOVERY_START",
            start.display().to_string(),
            "Product Model owner accepts only a directory or file path. Remedy: pass --path to an authored product root.",
        ));
    };
    let mut current = fs::canonicalize(start).map_err(|error| {
        workflow_error(
            "RUSTY_PRODUCT_DISCOVERY_START",
            start.display().to_string(),
            format!(
                "Product Model owner cannot resolve the explicit product path: {error}. Remedy: repair the path and try again."
            ),
        )
    })?;
    for _ in 0..MAX_DISCOVERY_ANCESTORS {
        let manifest = current.join(MANIFEST_NAME);
        match fs::symlink_metadata(&manifest) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(workflow_error(
                    "RUSTY_PRODUCT_MANIFEST_SYMLINK",
                    manifest.display().to_string(),
                    "Product Model owner does not admit a symlinked rusty.toml. Remedy: keep the manifest as a regular file inside the explicit product root.",
                ));
            }
            Ok(metadata) if metadata.is_file() => return Ok(current),
            Ok(_) | Err(_) => {}
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent.to_path_buf();
    }
    Err(workflow_error(
        "RUSTY_PRODUCT_ROOT_NOT_FOUND",
        start.display().to_string(),
        format!(
            "Product Model owner found no regular {MANIFEST_NAME} within {MAX_DISCOVERY_ANCESTORS} ancestors. Remedy: create or select an authored Rusty product root."
        ),
    ))
}

fn read_manifest(root: &Path) -> Result<ProductManifest, Diagnostic> {
    let manifest_path = root.join(MANIFEST_NAME);
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        workflow_error(
            "RUSTY_PRODUCT_MANIFEST_READ",
            MANIFEST_NAME,
            format!("Product Model owner cannot read rusty.toml: {error}. Remedy: restore a regular manifest file."),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(workflow_error(
            "RUSTY_PRODUCT_MANIFEST_KIND",
            MANIFEST_NAME,
            "Product Model owner requires rusty.toml to be a regular non-symlink file. Remedy: replace the manifest with an in-root regular file.",
        ));
    }
    if metadata.len() > MAX_PRODUCT_MANIFEST_BYTES as u64 {
        return Err(workflow_error(
            "RUSTY_PRODUCT_MANIFEST_BOUNDS",
            MANIFEST_NAME,
            format!(
                "Product Model owner limits rusty.toml to {MAX_PRODUCT_MANIFEST_BYTES} bytes. Remedy: keep Product Model policy compact and move content into its declared lanes."
            ),
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(&manifest_path)
        .and_then(|mut file| {
            file.by_ref()
                .take(MAX_PRODUCT_MANIFEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| {
            workflow_error(
                "RUSTY_PRODUCT_MANIFEST_READ",
                MANIFEST_NAME,
                format!("Product Model owner cannot read rusty.toml: {error}. Remedy: repair the manifest file permissions or contents."),
            )
        })?;
    if bytes.len() > MAX_PRODUCT_MANIFEST_BYTES {
        return Err(workflow_error(
            "RUSTY_PRODUCT_MANIFEST_BOUNDS",
            MANIFEST_NAME,
            format!(
                "Product Model owner limits rusty.toml to {MAX_PRODUCT_MANIFEST_BYTES} bytes. Remedy: keep Product Model policy compact and move content into its declared lanes."
            ),
        ));
    }
    let manifest_text = String::from_utf8(bytes).map_err(|error| {
        workflow_error(
            "RUSTY_PRODUCT_MANIFEST_UTF8",
            MANIFEST_NAME,
            format!("Product Model owner requires UTF-8 rusty.toml: {error}. Remedy: save the manifest as UTF-8."),
        )
    })?;
    decode_product_manifest(&manifest_text).map_err(|error| {
        let diagnostic = error.diagnostic();
        workflow_error(
            "RUSTY_PRODUCT_MANIFEST_ADMISSION",
            diagnostic.path(),
            format!(
                "Product Model owner rejected rusty.toml ({}) from {}: {}. Remedy: correct the declared product policy before generating runtime output.",
                diagnostic.code(),
                diagnostic.source(),
                diagnostic.message()
            ),
        )
    })
}

struct EngineCheckout {
    root: PathBuf,
    facade: PathBuf,
    product_kernel: PathBuf,
}

fn current_engine_checkout() -> Result<EngineCheckout, Diagnostic> {
    let cli = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = cli
        .ancestors()
        .nth(3)
        .ok_or_else(|| {
            workflow_error(
                "RUSTY_ENGINE_CHECKOUT",
                cli.display().to_string(),
                "Rusty CLI was not built inside a recognizable Engine checkout. Remedy: run the CLI supplied by the current Rusty Engine source tree.",
            )
        })?
        .to_path_buf();
    let root = fs::canonicalize(root).map_err(|error| {
        workflow_error(
            "RUSTY_ENGINE_CHECKOUT",
            "rusty-cli",
            format!("cannot resolve current Rusty Engine checkout: {error}. Remedy: rebuild Rusty CLI from a complete Engine checkout."),
        )
    })?;
    let facade = root.join("rust/crates/rusty-engine");
    let product_kernel = root.join("rust/crates/product-kernel");
    for path in [&facade, &product_kernel] {
        if !path.join("Cargo.toml").is_file() {
            return Err(workflow_error(
                "RUSTY_ENGINE_CHECKOUT",
                path.display().to_string(),
                "Rusty CLI's current Engine checkout is incomplete. Remedy: restore the Engine Rust crates before building a product.",
            ));
        }
    }
    Ok(EngineCheckout {
        root,
        facade,
        product_kernel,
    })
}

fn materialization_toolchain(
    engine: &EngineCheckout,
) -> Result<MaterializationToolchain, Diagnostic> {
    let node = find_program("node").ok_or_else(|| {
        workflow_error(
            "RUSTY_MATERIALIZER_NODE",
            "node",
            "Engine materializer owner cannot find Node on PATH. Remedy: install the Engine-supported Node runtime and rerun the command.",
        )
    })?;
    let typescript = engine
        .root
        .join("rules/node_modules/typescript/lib/typescript.js");
    let vite = engine.root.join("render/node_modules/vite/bin/vite.js");
    for (path, label) in [(&typescript, "TypeScript"), (&vite, "Vite")] {
        if !path.is_file() {
            return Err(workflow_error(
                "RUSTY_MATERIALIZER_TOOLCHAIN",
                path.display().to_string(),
                format!("Engine materializer owner cannot find the current Engine {label} artifact. Remedy: install this checkout's isolated workspace dependencies before building a product."),
            ));
        }
    }
    Ok(MaterializationToolchain::new(node, typescript, vite)
        .with_temporary_parent(engine.root.join("target")))
}

fn find_program(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|directory| directory.join(name))
        .find(|candidate| matches!(fs::metadata(candidate), Ok(metadata) if metadata.is_file()))
}

#[derive(Debug, Deserialize)]
struct EnginePackageManifest {
    #[serde(default)]
    files: Vec<String>,
}

fn collect_engine_assets(engine: &EngineCheckout) -> Result<EngineAssets, Diagnostic> {
    let packages = [
        (
            "@rusty-engine/runtime-composition-authoring",
            engine
                .root
                .join("rules/packages/runtime-composition-authoring"),
        ),
        (
            "@rusty-engine/application-host",
            engine.root.join("render/artifacts/application-host"),
        ),
        (
            "@rusty-engine/product-browser-host",
            engine.root.join("render/artifacts/product-browser-host"),
        ),
    ];
    let mut assets = Vec::new();
    for (package, root) in packages {
        let package_bytes = read_engine_artifact(&root, Path::new("package.json"))?;
        let manifest: EnginePackageManifest = serde_json::from_slice(&package_bytes).map_err(|error| {
            workflow_error(
                "RUSTY_ENGINE_ARTIFACT_MANIFEST",
                root.join("package.json").display().to_string(),
                format!("Engine materializer owner cannot decode the {package} artifact manifest: {error}. Remedy: rebuild the current Engine artifact before product generation."),
            )
        })?;
        assets.push(
            EngineAsset::new(package, "package.json", package_bytes)
                .map_err(materializer_diagnostic)?,
        );
        let mut paths = BTreeSet::new();
        for declared in manifest.files {
            collect_declared_artifact_paths(&root, Path::new(&declared), &mut paths)?;
        }
        for path in paths {
            if path == Path::new("package.json") {
                continue;
            }
            let logical = path.to_string_lossy().replace('\\', "/");
            assets.push(
                EngineAsset::new(package, logical, read_engine_artifact(&root, &path)?)
                    .map_err(materializer_diagnostic)?,
            );
        }
    }
    EngineAssets::new(assets).map_err(materializer_diagnostic)
}

fn collect_declared_artifact_paths(
    package_root: &Path,
    declared: &Path,
    collected: &mut BTreeSet<PathBuf>,
) -> Result<(), Diagnostic> {
    if !is_relative_normal_path(declared) {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_PATH",
            declared.display().to_string(),
            "Engine materializer owner found an invalid package artifact path. Remedy: rebuild the current Engine artifact from its checked package manifest.",
        ));
    }
    let path = package_root.join(declared);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        workflow_error(
            "RUSTY_ENGINE_ARTIFACT_READ",
            path.display().to_string(),
            format!("Engine materializer owner cannot read a declared artifact: {error}. Remedy: rebuild the current Engine artifact."),
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_SYMLINK",
            path.display().to_string(),
            "Engine materializer owner does not admit symlinked Engine artifacts. Remedy: rebuild the checked artifact in this Engine checkout.",
        ));
    }
    if metadata.is_file() {
        if collected.len() >= MAX_ENGINE_ARTIFACT_ENTRIES {
            return Err(workflow_error(
                "RUSTY_ENGINE_ARTIFACT_BOUNDS",
                package_root.display().to_string(),
                "Engine materializer owner reached its artifact-entry bound. Remedy: keep the published Engine package closure bounded.",
            ));
        }
        collected.insert(declared.to_path_buf());
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_KIND",
            path.display().to_string(),
            "Engine materializer owner requires package artifacts to be regular files or directories. Remedy: rebuild the checked Engine artifact.",
        ));
    }
    let mut entries = fs::read_dir(&path)
        .map_err(|error| {
            workflow_error(
                "RUSTY_ENGINE_ARTIFACT_READ",
                path.display().to_string(),
                format!("Engine materializer owner cannot enumerate declared artifacts: {error}. Remedy: rebuild the current Engine artifact."),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            workflow_error(
                "RUSTY_ENGINE_ARTIFACT_READ",
                path.display().to_string(),
                format!("Engine materializer owner cannot enumerate declared artifacts: {error}. Remedy: rebuild the current Engine artifact."),
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            workflow_error(
                "RUSTY_ENGINE_ARTIFACT_PATH",
                path.display().to_string(),
                "Engine materializer owner requires UTF-8 artifact paths. Remedy: rebuild the checked Engine artifact.",
            )
        })?;
        collect_declared_artifact_paths(package_root, &declared.join(name), collected)?;
    }
    Ok(())
}

fn is_relative_normal_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn read_engine_artifact(package_root: &Path, relative: &Path) -> Result<Vec<u8>, Diagnostic> {
    let path = package_root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        workflow_error(
            "RUSTY_ENGINE_ARTIFACT_READ",
            path.display().to_string(),
            format!("Engine materializer owner cannot inspect artifact: {error}. Remedy: rebuild the current Engine artifact."),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_KIND",
            path.display().to_string(),
            "Engine materializer owner requires regular non-symlink artifact files. Remedy: rebuild the current Engine artifact.",
        ));
    }
    if metadata.len() > MAX_ENGINE_ARTIFACT_FILE_BYTES {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_BOUNDS",
            path.display().to_string(),
            "Engine materializer owner found an artifact larger than 2 MiB. Remedy: publish a bounded Engine artifact closure.",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    fs::File::open(&path)
        .and_then(|mut file| {
            file.by_ref()
                .take(MAX_ENGINE_ARTIFACT_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|error| {
            workflow_error(
                "RUSTY_ENGINE_ARTIFACT_READ",
                path.display().to_string(),
                format!("Engine materializer owner cannot read artifact: {error}. Remedy: rebuild the current Engine artifact."),
            )
        })?;
    if bytes.len() as u64 > MAX_ENGINE_ARTIFACT_FILE_BYTES {
        return Err(workflow_error(
            "RUSTY_ENGINE_ARTIFACT_BOUNDS",
            path.display().to_string(),
            "Engine materializer owner found an artifact larger than 2 MiB. Remedy: publish a bounded Engine artifact closure.",
        ));
    }
    Ok(bytes)
}

fn cargo_relative_path(from: &Path, to: &Path) -> Result<String, Diagnostic> {
    let to = fs::canonicalize(to).map_err(|error| {
        workflow_error(
            "RUSTY_GENERATED_DEPENDENCY_PATH",
            to.display().to_string(),
            format!("Product Assembly owner cannot resolve its Engine dependency: {error}. Remedy: restore the current Engine checkout."),
        )
    })?;
    cargo_relative_path_from_normalized(from, &to)
}

/// Computes a path from the `Cargo.toml` directory itself.  The generated
/// assembly directory may not exist until publication, so this intentionally
/// stays lexical on the already-canonical product-root path.
fn cargo_relative_path_from_normalized(from: &Path, to: &Path) -> Result<String, Diagnostic> {
    let from_components = normal_components(from)?;
    let to_components = normal_components(to)?;
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    let mut pieces = Vec::new();
    pieces.extend(std::iter::repeat_n(
        "..".to_owned(),
        from_components.len() - common,
    ));
    pieces.extend(to_components[common..].iter().cloned());
    if pieces.is_empty() {
        return Err(workflow_error(
            "RUSTY_GENERATED_DEPENDENCY_PATH",
            to.display().to_string(),
            "Product Assembly owner resolved an ambiguous same-directory dependency. Remedy: keep generated assembly separate from Engine crates.",
        ));
    }
    Ok(pieces.join("/"))
}

fn normal_components(path: &Path) -> Result<Vec<String>, Diagnostic> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) => {}
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::CurDir | Component::ParentDir => {
                return Err(workflow_error(
                    "RUSTY_GENERATED_DEPENDENCY_PATH",
                    path.display().to_string(),
                    "Product Assembly owner requires normalized absolute paths. Remedy: choose a stable explicit product and Engine checkout path.",
                ));
            }
        }
    }
    Ok(parts)
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    manifest_path: PathBuf,
    targets: Vec<CargoTarget>,
}

#[derive(Debug, Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

impl CargoMetadata {
    fn target_for(&self, cargo_manifest: &Path) -> Result<&CargoTarget, Diagnostic> {
        let manifest = fs::canonicalize(cargo_manifest).map_err(|error| {
            workflow_error(
                "RUSTY_GENERATED_METADATA",
                cargo_manifest.display().to_string(),
                format!("Generated Product Assembly owner cannot resolve Cargo manifest: {error}. Remedy: republish the Product Assembly."),
            )
        })?;
        let package = self
            .packages
            .iter()
            .find(|package| package.manifest_path == manifest)
            .ok_or_else(|| {
                workflow_error(
                    "RUSTY_GENERATED_METADATA",
                    cargo_manifest.display().to_string(),
                    "Generated Product Assembly owner could not find its detached Cargo package. Remedy: republish the Product Assembly.",
                )
            })?;
        let targets = package
            .targets
            .iter()
            .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
            .collect::<Vec<_>>();
        if targets.len() != 1 {
            return Err(workflow_error(
                "RUSTY_GENERATED_METADATA",
                cargo_manifest.display().to_string(),
                "Generated Product Assembly owner requires exactly one Cargo binary target. Remedy: regenerate the Engine-owned assembly rather than editing it.",
            ));
        }
        Ok(targets[0])
    }
}

fn cargo_metadata(cargo_manifest: &Path) -> Result<CargoMetadata, Diagnostic> {
    let mut command = Command::new("cargo");
    command
        .arg("metadata")
        .arg("--offline")
        .arg("--no-deps")
        .arg("--format-version=1")
        .arg("--manifest-path")
        .arg(cargo_manifest)
        .current_dir(cargo_manifest.parent().unwrap_or_else(|| Path::new(".")))
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = run_bounded(command, CARGO_TIMEOUT).map_err(process_diagnostic)?;
    if !output.status.success() {
        return Err(workflow_error(
            "RUSTY_GENERATED_METADATA",
            "generated/product-assembly/Cargo.toml",
            format!(
                "Generated Product Assembly owner could not inspect Cargo metadata. Source: Cargo diagnostics: {}. Remedy: republish the generated assembly and correct its declared Product Kernel contract.",
                output_text(&output.stderr)
            ),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| {
        workflow_error(
            "RUSTY_GENERATED_METADATA",
            "generated/product-assembly/Cargo.toml",
            format!("Generated Product Assembly owner received invalid bounded Cargo metadata: {error}. Remedy: rerun `rusty build` from a supported Cargo installation."),
        )
    })
}

fn workflow_error(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(code, path, message)
}

fn materializer_diagnostic(error: product_materializer::MaterializationError) -> Diagnostic {
    let diagnostic = error.diagnostic();
    workflow_error(
        "RUSTY_MATERIALIZATION",
        diagnostic.path(),
        format!(
            "Engine materializer owner rejected authored product input ({}) at {}: {}. Remedy: keep Runtime Composition and UI source inside their declared lanes and use only the supported Engine authoring/UI imports.",
            diagnostic.code(),
            diagnostic.path(),
            diagnostic.message()
        ),
    )
}

fn assembly_diagnostic(error: product_assembly::ProductAssemblyError) -> Diagnostic {
    let diagnostic = error.diagnostic();
    workflow_error(
        "RUSTY_PRODUCT_ASSEMBLY",
        diagnostic.path(),
        format!(
            "Product Assembly owner rejected the admitted closure ({}) at {}: {}. Remedy: correct authored content, Product Kernel linkage, or generated-output drift and rerun the command.",
            diagnostic.code(),
            diagnostic.path(),
            diagnostic.message()
        ),
    )
}

fn probe_diagnostic(error: KernelProbeError) -> Diagnostic {
    workflow_error(
        "RUSTY_PRODUCT_KERNEL_PROBE",
        error.path(),
        format!(
            "Product Kernel owner could not provide its fixed RustyProductRuntime capability catalog ({}): {}. Remedy: make kernel.entry compile against the current Engine facade and expose the closed runtime definition.",
            error.code(),
            error.message()
        ),
    )
}

fn process_diagnostic(error: KernelProbeError) -> Diagnostic {
    workflow_error(
        error.code(),
        error.path(),
        format!(
            "Rusty CLI process owner could not complete a bounded Engine command: {}. Remedy: inspect the local tool installation and rerun the command.",
            error.message()
        ),
    )
}

fn output_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_owned();
    if text.is_empty() {
        "no diagnostic text was emitted".to_owned()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{admit_product, cargo_relative_path_from_normalized};

    #[test]
    fn generated_assembly_dependency_path_is_anchored_on_its_cargo_directory() {
        let path = cargo_relative_path_from_normalized(
            Path::new("/workspace/product/generated/product-assembly"),
            Path::new("/workspace/rusty-engine/rust/crates/rusty-engine"),
        )
        .expect("normalized dependency path");
        assert_eq!(path, "../../../rusty-engine/rust/crates/rusty-engine");
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; scripts/verify-product-conformance.sh owns this integration proof"]
    fn conformance_product_admits_through_compiled_kernel_probe_without_generation() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("fixtures/product-conformance");
        let admitted = admit_product(root).expect("conformance Product Model admission");
        assert_eq!(
            admitted.manifest().product_id(),
            "rusty.product.conformance"
        );
        assert_eq!(
            admitted
                .kernel_capabilities()
                .iter()
                .map(|capability| capability.identity())
                .collect::<Vec<_>>(),
            [
                "counter-increment",
                "counter-observe",
                "counter-recurring",
                "counter-recurring-result",
                "counter-timeline",
            ]
        );
    }
}
