use cap_primitives::fs::FollowSymlinks;
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs, io,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use product_assembly::{
    AssemblyGenerationInputs, BrowserBundleInputs, ProductAssemblyError, PublicationFile,
};
use product_model::{
    decode_compiled_composition, encode_compiled_composition, LifecycleMode, ProductManifest,
    ProductPath,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MAX_FILES: usize = 512;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_TOOLING_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ENGINE_ASSETS: usize = 1_024;
const MAX_TOOL_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TOOLING_FILES: usize = 4_096;
const MAX_TOOLING_TOTAL_BYTES: usize = 64 * 1024 * 1024;

/// One content-addressed Engine build asset. `relative_path` is rooted at the
/// package identity (for example `@rusty-engine/product-browser-host/index.js`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineAsset {
    package: String,
    relative_path: String,
    bytes: Vec<u8>,
    sha256: String,
}

impl EngineAsset {
    pub fn new(
        package: impl Into<String>,
        relative_path: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, MaterializationError> {
        let package = package.into();
        let relative_path = relative_path.into();
        validate_package(&package)?;
        validate_relative(&relative_path, "engine-assets")?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(fail(
                "MATERIALIZER_ENGINE_ASSET_BOUNDS",
                &relative_path,
                "Engine asset exceeds the per-file bound",
            ));
        }
        let sha256 = sha256(&bytes);
        Ok(Self {
            package,
            relative_path,
            bytes,
            sha256,
        })
    }
    pub fn package(&self) -> &str {
        &self.package
    }
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Immutable, verified Engine inputs. These are byte assets rather than paths
/// so a temporary build workspace cannot accidentally become a runtime input.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineAssets {
    assets: Vec<EngineAsset>,
}
impl EngineAssets {
    pub fn new(assets: Vec<EngineAsset>) -> Result<Self, MaterializationError> {
        if assets.len() > MAX_ENGINE_ASSETS {
            return Err(fail(
                "MATERIALIZER_ENGINE_ASSET_BOUNDS",
                "engine-assets",
                "Engine asset count exceeds bound",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut total = 0usize;
        for asset in &assets {
            if !seen.insert((asset.package.clone(), asset.relative_path.clone())) {
                return Err(fail(
                    "MATERIALIZER_ENGINE_ASSET_DUPLICATE",
                    asset.relative_path(),
                    "duplicate Engine asset",
                ));
            }
            if asset.sha256 != sha256(&asset.bytes) {
                return Err(fail(
                    "MATERIALIZER_ENGINE_ASSET_HASH",
                    asset.relative_path(),
                    "Engine asset hash did not match its immutable bytes",
                ));
            }
            total = total.checked_add(asset.bytes.len()).ok_or_else(|| {
                fail(
                    "MATERIALIZER_ENGINE_ASSET_BOUNDS",
                    "engine-assets",
                    "aggregate overflow",
                )
            })?;
        }
        if total > MAX_TOTAL_BYTES {
            return Err(fail(
                "MATERIALIZER_ENGINE_ASSET_BOUNDS",
                "engine-assets",
                "Engine assets exceed aggregate bound",
            ));
        }
        Ok(Self { assets })
    }
    pub fn assets(&self) -> &[EngineAsset] {
        &self.assets
    }
}

#[derive(Debug, Clone)]
pub struct MaterializationToolchain {
    pub node: PathBuf,
    pub typescript_module: PathBuf,
    pub vite: PathBuf,
    temporary_parent: Option<PathBuf>,
}
impl MaterializationToolchain {
    pub fn new(
        node: impl Into<PathBuf>,
        typescript_module: impl Into<PathBuf>,
        vite: impl Into<PathBuf>,
    ) -> Self {
        Self {
            node: node.into(),
            typescript_module: typescript_module.into(),
            vite: vite.into(),
            temporary_parent: None,
        }
    }
    /// Selects a tool-owned scratch parent. The temporary child is always
    /// removed; this does not grant the authored evaluators more authority.
    pub fn with_temporary_parent(mut self, parent: impl Into<PathBuf>) -> Self {
        self.temporary_parent = Some(parent.into());
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MaterializationLimits {
    pub timeout: Duration,
}
impl Default for MaterializationLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedProduct {
    compiled_composition: Vec<u8>,
    browser_bundle_files: Vec<PublicationFile>,
    runtime_program: Option<Vec<u8>>,
}
impl MaterializedProduct {
    pub fn compiled_composition(&self) -> &[u8] {
        &self.compiled_composition
    }
    pub fn browser_bundle_files(&self) -> &[PublicationFile] {
        &self.browser_bundle_files
    }

    /// The optional single-file program for the Engine-owned runtime VM.
    /// Product Assembly integration remains a separate owner boundary.
    pub fn runtime_program(&self) -> Option<&[u8]> {
        self.runtime_program.as_deref()
    }

    /// Converts the immutable materialization receipt into the exact typed
    /// fresh input required by Product Assembly; no generated output is read.
    pub fn assembly_inputs(&self) -> Result<AssemblyGenerationInputs, ProductAssemblyError> {
        let bundle = BrowserBundleInputs::new("ui/main.js", self.browser_bundle_files.clone())?;
        let mut inputs = AssemblyGenerationInputs::new(self.compiled_composition.clone())?
            .with_browser_bundle(bundle)?;
        if let Some(program) = &self.runtime_program {
            inputs = inputs.with_runtime_program(program.clone())?;
        }
        Ok(inputs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationDiagnostic {
    code: String,
    path: String,
    message: String,
}
impl MaterializationDiagnostic {
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn message(&self) -> &str {
        &self.message
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationError {
    diagnostic: MaterializationDiagnostic,
}
impl MaterializationError {
    pub fn diagnostic(&self) -> &MaterializationDiagnostic {
        &self.diagnostic
    }
}
impl std::fmt::Display for MaterializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}: {}",
            self.diagnostic.code, self.diagnostic.path, self.diagnostic.message
        )
    }
}
impl std::error::Error for MaterializationError {}
fn fail(code: &str, path: &str, message: impl Into<String>) -> MaterializationError {
    MaterializationError {
        diagnostic: MaterializationDiagnostic {
            code: code.into(),
            path: path.into(),
            message: bound(message.into()),
        },
    }
}

/// Rebuilds the two runtime inputs from a product's declared authored lanes.
/// The temporary directory is always removed; a removal failure is reported as
/// an error so stale source closures are never silently retained.
pub fn materialize_product(
    root: &Path,
    manifest: &ProductManifest,
    assets: &EngineAssets,
    toolchain: &MaterializationToolchain,
    limits: MaterializationLimits,
) -> Result<MaterializedProduct, MaterializationError> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("rusty-product-materialize-");
    let temporary = match toolchain.temporary_parent.as_deref() {
        Some(parent) => builder.tempdir_in(parent),
        None => builder.tempdir(),
    }
    .map_err(|e| fail("MATERIALIZER_TEMP_CREATE", "temporary", e.to_string()))?;
    let result = materialize_in(root, manifest, assets, toolchain, limits, temporary.path());
    let cleanup = temporary.close();
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(fail(
            "MATERIALIZER_TEMP_CLEANUP",
            "temporary",
            error.to_string(),
        )),
        (Err(error), Err(cleanup)) => Err(fail(
            "MATERIALIZER_TEMP_CLEANUP",
            "temporary",
            format!("{error}; cleanup also failed: {cleanup}"),
        )),
    }
}

fn materialize_in(
    root: &Path,
    manifest: &ProductManifest,
    assets: &EngineAssets,
    toolchain: &MaterializationToolchain,
    limits: MaterializationLimits,
    temp: &Path,
) -> Result<MaterializedProduct, MaterializationError> {
    if limits.timeout.is_zero() || limits.timeout > MAX_TOOL_TIMEOUT {
        return Err(fail(
            "MATERIALIZER_TIMEOUT_BOUNDS",
            "limits.timeout",
            "tool timeout must be within 1ms..=120s",
        ));
    }
    if !root.is_dir() {
        return Err(fail(
            "MATERIALIZER_ROOT",
            "product-root",
            "product root is not a directory",
        ));
    }
    for program in [
        &toolchain.node,
        &toolchain.typescript_module,
        &toolchain.vite,
    ] {
        if !program.exists() {
            return Err(fail(
                "MATERIALIZER_TOOL_MISSING",
                "toolchain",
                format!("required build tool is missing: {}", program.display()),
            ));
        }
    }
    let authored_root = open_authored_root(root)?;
    verify_node_permission_support(&toolchain.node, temp)?;
    fs::write(temp.join("package.json"), "{\"type\":\"module\"}\n")
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &temp.join("package.json"), e))?;
    let src = temp.join("src");
    let node_modules = temp.join("node_modules");
    let out = temp.join("out");
    fs::create_dir_all(&src).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &src, e))?;
    fs::create_dir_all(&node_modules)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &node_modules, e))?;
    copy_engine_assets(assets, &node_modules)?;
    let tooling = temp.join("tooling");
    fs::create_dir_all(&tooling).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &tooling, e))?;
    let typescript_root = tooling.join("typescript");
    let typescript_package = toolchain
        .typescript_module
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            fail(
                "MATERIALIZER_TOOL_COPY",
                "toolchain",
                "TypeScript module has no package directory",
            )
        })?;
    let typescript_package = fs::canonicalize(typescript_package)
        .map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", typescript_package, e))?;
    copy_tooling_tree(&typescript_package, &typescript_root)?;
    let typescript = typescript_root.join("lib/typescript.js");
    let scanner = temp.join("scan.mjs");
    fs::write(&scanner, SCANNER).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &scanner, e))?;
    let authoring_allowed =
        BTreeSet::from(["@rusty-engine/runtime-composition-authoring".to_string()]);
    let ui_allowed = BTreeSet::from([
        "@rusty-engine/application-host".to_string(),
        "@rusty-engine/product-browser-host".to_string(),
    ]);
    let mut copied = BTreeSet::new();
    let mut closure_compiler = ClosureCompiler {
        authored_root: &authored_root,
        destination: &src,
        scanner: &scanner,
        node: &toolchain.node,
        typescript: &typescript,
        limits,
        copied: &mut copied,
        source_bytes: 0,
    };
    for entry in manifest.composition_entrypoints() {
        closure_compiler.copy(entry, &authoring_allowed)?;
    }
    if let Some(entry) = manifest.runtime_entry() {
        closure_compiler.copy(entry, &BTreeSet::new())?;
    }
    closure_compiler.copy(manifest.ui_entry(), &ui_allowed)?;
    let rules_typecheck = temp.join("rules-typecheck.mjs");
    fs::write(&rules_typecheck, TYPECHECKER)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &rules_typecheck, e))?;
    let output = run_trusted_vite(
        &toolchain.node,
        &rules_typecheck,
        &[
            typescript.as_os_str(),
            src.join("rules").as_os_str(),
            OsStr::new("rules"),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RULES_TYPECHECK", "rules", output)?;
    let rules_compile = temp.join("rules-compile.mjs");
    let compiled_rules = temp.join("compiled-rules");
    fs::write(&rules_compile, RULES_COMPILER)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &rules_compile, e))?;
    let output = run_node(
        &toolchain.node,
        &rules_compile,
        &[
            typescript.as_os_str(),
            src.join("rules").as_os_str(),
            compiled_rules.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RULES_COMPILE", "rules", output)?;
    let runner = temp.join("rules-runner.mjs");
    fs::write(&runner, RULES_RUNNER).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &runner, e))?;
    let composition_json = out.join("compiled-composition.json");
    fs::create_dir_all(&out).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &out, e))?;
    let entries = manifest
        .composition_entrypoints()
        .iter()
        .map(|p| {
            compiled_rules
                .join(p.as_str().strip_prefix("rules/").unwrap_or(p.as_str()))
                .with_extension("js")
                .to_string_lossy()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let output = run_node(
        &toolchain.node,
        &runner,
        &[
            OsStr::new(manifest.product_id()),
            OsStr::new(&entries),
            composition_json.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RULES_FAILED", "rules", output)?;
    let raw = fs::read(&composition_json)
        .map_err(|e| io_fail("MATERIALIZER_COMPOSITION_MISSING", &composition_json, e))?;
    let composition = decode_compiled_composition(&raw).map_err(|e| {
        fail(
            "MATERIALIZER_COMPOSITION_ADMISSION",
            "compiled-composition.json",
            e.to_string(),
        )
    })?;
    if composition.candidate().product != manifest.product_id() {
        return Err(fail(
            "MATERIALIZER_PRODUCT_ID",
            "compiled-composition.json",
            "rules composition product did not match rusty.toml",
        ));
    }
    let compiled_composition = encode_compiled_composition(&composition);
    let runtime_program = match manifest.runtime_entry() {
        Some(entry) => Some(bundle_runtime_program(
            entry,
            temp,
            &toolchain.node,
            &toolchain.vite,
            &typescript,
            &scanner,
            limits,
        )?),
        None => None,
    };
    let ui_out = out.join("ui");
    let config = temp.join("vite.config.mjs");
    fs::write(&config, vite_config(manifest.ui_entry().as_str(), &ui_out))
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &config, e))?;
    let typecheck = temp.join("typecheck.mjs");
    fs::write(&typecheck, TYPECHECKER)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &typecheck, e))?;
    let rules_check = run_node(
        &toolchain.node,
        &typecheck,
        &[
            typescript.as_os_str(),
            src.join("rules").as_os_str(),
            OsStr::new("rules"),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RULES_TYPECHECK", "rules", rules_check)?;
    let ui_harness = temp.join("ui-contract.ts");
    fs::write(
        &ui_harness,
        format!(
            "import {{ mountProductUi }} from './src/{}';\nimport type {{ RustyApplicationUiMount }} from '@rusty-engine/application-host';\nconst checked: RustyApplicationUiMount = mountProductUi;\nvoid checked;\n",
            manifest.ui_entry().as_str().replace(".ts", ".js")
        ),
    ).map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &ui_harness, e))?;
    let ui_check = run_node(
        &toolchain.node,
        &typecheck,
        &[
            typescript.as_os_str(),
            src.join("ui").as_os_str(),
            OsStr::new("ui"),
            ui_harness.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_UI_TYPECHECK", "ui", ui_check)?;
    // Vite is an Engine-selected build tool, not authored product code. It is
    // run as a trusted tool only after the copied source closure and fixed
    // configuration have passed the restricted scanner/typecheck lanes.
    let output = run_trusted_node(
        &toolchain.node,
        &toolchain.vite,
        &[
            OsStr::new("build"),
            OsStr::new("--config"),
            config.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_UI_BUILD", "ui", output)?;
    let mut browser_bundle_files = collect_output(&ui_out, "ui")?;
    let bundle_runner = temp.join("bundle-runner.mjs");
    fs::write(&bundle_runner, BUNDLE_RUNNER)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &bundle_runner, e))?;
    let bundle_descriptor = out.join("bundle-descriptor.json");
    let _engine_module = assets
        .assets()
        .iter()
        .find(|asset| {
            asset.package == "@rusty-engine/product-browser-host"
                && asset.relative_path == "product-browser-host.js"
        })
        .ok_or_else(|| {
            fail(
                "MATERIALIZER_BROWSER_HOST_ASSET",
                "engine-assets",
                "missing immutable product-browser-host.js artifact",
            )
        })?;
    let output = run_node(
        &toolchain.node,
        &bundle_runner,
        &[
            OsStr::new(lifecycle_name(manifest.lifecycle())),
            OsStr::new(manifest.ui_projection_stream().unwrap_or("")),
            OsStr::new(manifest.ui_projection_contract().unwrap_or("")),
            bundle_descriptor.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_BROWSER_DESCRIPTOR", "product-bundle", output)?;
    let descriptor: BundleDescriptor = serde_json::from_slice(
        &fs::read(&bundle_descriptor)
            .map_err(|e| io_fail("MATERIALIZER_BROWSER_DESCRIPTOR", &bundle_descriptor, e))?,
    )
    .map_err(|e| {
        fail(
            "MATERIALIZER_BROWSER_DESCRIPTOR",
            "product-bundle",
            e.to_string(),
        )
    })?;
    if descriptor.artifact != "rusty.product.bundle" {
        return Err(fail(
            "MATERIALIZER_BROWSER_DESCRIPTOR",
            "product-bundle",
            "browser bundle descriptor artifact mismatch",
        ));
    }
    let expected_browser_roots = [
        "index.html",
        "main.js",
        "bridge.js",
        "engine/product-browser-host.js",
    ];
    if descriptor.files.len() != expected_browser_roots.len() {
        return Err(fail(
            "MATERIALIZER_BROWSER_DESCRIPTOR",
            "product-bundle",
            "browser descriptor must contain exactly its fixed Engine roots",
        ));
    }
    for (index, asset) in descriptor.files.into_iter().enumerate() {
        if asset.name != expected_browser_roots[index]
            || asset.content.is_empty()
            || asset.utf8_bytes != asset.content.len()
        {
            return Err(fail(
                "MATERIALIZER_BROWSER_DESCRIPTOR",
                "product-bundle",
                "browser descriptor roots, byte counts, and order must be exact",
            ));
        }
        browser_bundle_files.push(
            PublicationFile::new(asset.name, asset.content.into_bytes())
                .map_err(|e| fail("MATERIALIZER_BUNDLE", "product-bundle", e.to_string()))?,
        );
    }
    browser_bundle_files.push(
        PublicationFile::new(
            "runtime-adapter.js",
            b"export const PRODUCT_RUNTIME_HTTP_BASE_PATH = '/__rusty/product/runtime/';\n"
                .to_vec(),
        )
        .map_err(|e| fail("MATERIALIZER_BUNDLE", "runtime-adapter.js", e.to_string()))?,
    );
    browser_bundle_files.sort_by(|a, b| a.relative_path().as_str().cmp(b.relative_path().as_str()));
    validate_emitted_modules(
        &browser_bundle_files,
        &temp.join("emitted-module-check"),
        &scanner,
        &toolchain.node,
        &typescript,
        temp,
        limits,
    )?;
    reject_runtime_leaks(&browser_bundle_files, temp)?;
    Ok(MaterializedProduct {
        compiled_composition,
        browser_bundle_files,
        runtime_program,
    })
}

fn bundle_runtime_program(
    entry: &ProductPath,
    temp: &Path,
    node: &Path,
    vite: &Path,
    typescript: &Path,
    scanner: &Path,
    limits: MaterializationLimits,
) -> Result<Vec<u8>, MaterializationError> {
    let typecheck = temp.join("runtime-typecheck.mjs");
    fs::write(&typecheck, TYPECHECKER)
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &typecheck, e))?;
    let source_entry = entry.as_str().replace(".ts", ".js");
    let contract = temp.join("runtime-contract.ts");
    fs::write(
        &contract,
        format!(
            "import runtime from './src/{source_entry}';\ntype RuntimeExport = {{ initialize: CallableFunction; turn: CallableFunction; project: CallableFunction; }};\nconst checked: RuntimeExport = runtime;\nvoid checked;\n"
        ),
    )
    .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &contract, e))?;
    let output = run_node(
        node,
        &typecheck,
        &[
            typescript.as_os_str(),
            temp.join("src/runtime").as_os_str(),
            OsStr::new("runtime"),
            contract.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RUNTIME_TYPECHECK", "runtime", output)?;

    let wrapper = temp.join("runtime-entry.ts");
    fs::write(
        &wrapper,
        format!(
            "import runtime from './src/{source_entry}';\nconst {{ initialize, turn, project }} = runtime;\nif (typeof initialize !== 'function' || typeof turn !== 'function' || typeof project !== 'function') throw new Error('runtime default export must provide initialize, turn, and project functions');\nglobalThis.__rustyEngineRuntime = {{ initialize, turn, project }};\n"
        ),
    )
    .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &wrapper, e))?;
    let runtime_out = temp.join("out/runtime");
    let config = temp.join("runtime-vite.config.mjs");
    fs::write(&config, runtime_vite_config(&wrapper, &runtime_out))
        .map_err(|e| io_fail("MATERIALIZER_TEMP_WRITE", &config, e))?;
    let output = run_trusted_node(
        node,
        vite,
        &[
            OsStr::new("build"),
            OsStr::new("--config"),
            config.as_os_str(),
        ],
        temp,
        limits,
    )?;
    check_output("MATERIALIZER_RUNTIME_BUILD", "runtime", output)?;
    let files = collect_output(&runtime_out, "runtime")?;
    if files.len() != 1 || !files[0].relative_path().as_str().ends_with(".js") {
        return Err(fail(
            "MATERIALIZER_RUNTIME_BUNDLE",
            "runtime",
            "runtime build must emit exactly one JavaScript program",
        ));
    }
    validate_emitted_modules(
        &files,
        &temp.join("runtime-module-check"),
        scanner,
        node,
        typescript,
        temp,
        limits,
    )?;
    reject_runtime_leaks(&files, temp)?;
    Ok(files[0].bytes().to_vec())
}

fn copy_engine_assets(
    assets: &EngineAssets,
    node_modules: &Path,
) -> Result<(), MaterializationError> {
    for asset in assets.assets() {
        let path = node_modules.join(&asset.package).join(&asset.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| io_fail("MATERIALIZER_ENGINE_ASSET_WRITE", parent, e))?;
        }
        fs::write(&path, asset.bytes())
            .map_err(|e| io_fail("MATERIALIZER_ENGINE_ASSET_WRITE", &path, e))?;
    }
    Ok(())
}

fn copy_tooling_tree(source: &Path, destination: &Path) -> Result<(), MaterializationError> {
    let mut files = 0usize;
    let mut bytes = 0usize;
    copy_tooling_tree_inner(source, destination, 0, &mut files, &mut bytes)
}

fn copy_tooling_tree_inner(
    source: &Path,
    destination: &Path,
    depth: usize,
    files: &mut usize,
    bytes: &mut usize,
) -> Result<(), MaterializationError> {
    if depth > 64 {
        return Err(fail(
            "MATERIALIZER_TOOL_COPY",
            source.to_string_lossy().as_ref(),
            "tooling tree exceeds depth bound",
        ));
    }
    let metadata =
        fs::symlink_metadata(source).map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", source, e))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(fail(
            "MATERIALIZER_TOOL_COPY",
            source.to_string_lossy().as_ref(),
            "tooling directory must be a regular non-symlink directory",
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", destination, e))?;
    for entry in fs::read_dir(source).map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", source, e))? {
        let entry = entry.map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", source, e))?;
        let input = entry.path();
        let output = destination.join(entry.file_name());
        let kind = entry
            .file_type()
            .map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", &input, e))?;
        if kind.is_symlink() {
            return Err(fail(
                "MATERIALIZER_TOOL_COPY",
                input.to_string_lossy().as_ref(),
                "tooling closure must not contain symlinks",
            ));
        }
        if kind.is_dir() {
            copy_tooling_tree_inner(&input, &output, depth + 1, files, bytes)?;
        } else if kind.is_file() {
            let length = entry
                .metadata()
                .map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", &input, e))?
                .len() as usize;
            *files = files.checked_add(1).ok_or_else(|| {
                fail(
                    "MATERIALIZER_TOOL_COPY",
                    "tooling",
                    "tooling file count overflow",
                )
            })?;
            *bytes = bytes.checked_add(length).ok_or_else(|| {
                fail("MATERIALIZER_TOOL_COPY", "tooling", "tooling byte overflow")
            })?;
            if *files > MAX_TOOLING_FILES
                || *bytes > MAX_TOOLING_TOTAL_BYTES
                || length > MAX_TOOLING_FILE_BYTES
            {
                return Err(fail(
                    "MATERIALIZER_TOOL_COPY",
                    input.to_string_lossy().as_ref(),
                    "tooling file, count, or aggregate bounds exceeded",
                ));
            }
            fs::copy(&input, &output).map_err(|e| io_fail("MATERIALIZER_TOOL_COPY", &output, e))?;
        }
    }
    Ok(())
}

struct ClosureCompiler<'a> {
    authored_root: &'a Dir,
    destination: &'a Path,
    scanner: &'a Path,
    node: &'a Path,
    typescript: &'a Path,
    limits: MaterializationLimits,
    copied: &'a mut BTreeSet<String>,
    source_bytes: usize,
}

impl ClosureCompiler<'_> {
    fn copy(
        &mut self,
        entry: &ProductPath,
        allowed: &BTreeSet<String>,
    ) -> Result<(), MaterializationError> {
        let authored_root = self.authored_root;
        let destination = self.destination;
        let scanner = self.scanner;
        let node = self.node;
        let typescript = self.typescript;
        let limits = self.limits;
        let copied = &mut self.copied;
        let mut pending = vec![entry.as_str().to_owned()];
        while let Some(relative) = pending.pop() {
            if !copied.insert(relative.clone()) {
                continue;
            }
            if copied.len() > MAX_FILES {
                return Err(fail(
                    "MATERIALIZER_SOURCE_BOUNDS",
                    &relative,
                    "source closure exceeds file bound",
                ));
            }
            validate_relative(&relative, "source")?;
            let bytes = read_authored_file(authored_root, &relative)?;
            self.source_bytes = self.source_bytes.checked_add(bytes.len()).ok_or_else(|| {
                fail(
                    "MATERIALIZER_SOURCE_BOUNDS",
                    &relative,
                    "source aggregate overflow",
                )
            })?;
            if self.source_bytes > MAX_TOTAL_BYTES {
                return Err(fail(
                    "MATERIALIZER_SOURCE_BOUNDS",
                    &relative,
                    "source closure exceeds aggregate byte bound",
                ));
            }
            let destination_file = destination.join(&relative);
            if let Some(parent) = destination_file.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| io_fail("MATERIALIZER_SOURCE_WRITE", parent, e))?;
            }
            fs::write(&destination_file, &bytes)
                .map_err(|e| io_fail("MATERIALIZER_SOURCE_WRITE", &destination_file, e))?;
            let scan = run_node(
                node,
                scanner,
                &[
                    typescript.as_os_str(),
                    destination_file.as_os_str(),
                    OsStr::new(entry.as_str().split('/').next().unwrap_or_default()),
                ],
                scanner.parent().unwrap_or(destination),
                limits,
            )?;
            let scan = check_output("MATERIALIZER_IMPORT_PARSE", &relative, scan)?;
            let imports: Imports = serde_json::from_slice(&scan.stdout)
                .map_err(|e| fail("MATERIALIZER_IMPORT_PARSE", &relative, e.to_string()))?;
            if let Some(violation) = imports.violations.first() {
                return Err(fail(
                    &violation.code,
                    &relative,
                    format!(
                        "{} (line {}, column {})",
                        violation.message, violation.line, violation.column
                    ),
                ));
            }
            if imports.dynamic {
                return Err(fail(
                    "MATERIALIZER_DYNAMIC_IMPORT",
                    &relative,
                    "dynamic import expressions are not admitted",
                ));
            }
            for specifier in imports.specifiers {
                if specifier.starts_with('.') {
                    if !specifier.ends_with(".js") {
                        return Err(fail(
                            "MATERIALIZER_IMPORT_SPECIFIER",
                            &relative,
                            "relative TypeScript imports must use their emitted .js specifier",
                        ));
                    }
                    let parent = Path::new(&relative)
                        .parent()
                        .unwrap_or_else(|| Path::new(""));
                    let joined = parent.join(specifier);
                    let normalized = normalize_relative(&joined)?;
                    let lane = entry.as_str().split('/').next().unwrap_or_default();
                    if normalized.split('/').next() != Some(lane) {
                        return Err(fail(
                            "MATERIALIZER_IMPORT_ESCAPE",
                            &relative,
                            "relative import crossed into another authored lane",
                        ));
                    }
                    pending.push(resolve_extension(authored_root, &normalized)?);
                } else if !allowed.contains(&specifier) {
                    return Err(fail(
                        "MATERIALIZER_BARE_IMPORT",
                        &relative,
                        format!("unadmitted bare import `{specifier}`"),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Imports {
    specifiers: Vec<String>,
    dynamic: bool,
    #[serde(default)]
    violations: Vec<SourcePolicyViolation>,
}

#[derive(Deserialize)]
struct SourcePolicyViolation {
    code: String,
    message: String,
    line: usize,
    column: usize,
}

fn resolve_extension(root: &Dir, relative: &str) -> Result<String, MaterializationError> {
    let authored_stem = relative
        .strip_suffix(".js")
        .or_else(|| relative.strip_suffix(".mjs"));
    for candidate in [
        relative.to_string(),
        authored_stem
            .map(|value| format!("{value}.ts"))
            .unwrap_or_default(),
        authored_stem
            .map(|value| format!("{value}.mts"))
            .unwrap_or_default(),
        authored_stem
            .map(|value| format!("{value}.tsx"))
            .unwrap_or_default(),
        format!("{relative}.ts"),
        format!("{relative}.mts"),
        format!("{relative}.tsx"),
        format!("{relative}.js"),
        format!("{relative}/index.ts"),
    ] {
        if authored_file_exists(root, &candidate)? {
            return Ok(candidate);
        }
    }
    Err(fail(
        "MATERIALIZER_IMPORT_MISSING",
        relative,
        "relative import did not resolve inside the authored lane",
    ))
}

fn open_authored_root(root: &Path) -> Result<Dir, MaterializationError> {
    let parent = root.parent().ok_or_else(|| {
        fail(
            "MATERIALIZER_SOURCE_ROOT",
            "product-root",
            "product root has no parent",
        )
    })?;
    let name = root.file_name().ok_or_else(|| {
        fail(
            "MATERIALIZER_SOURCE_ROOT",
            "product-root",
            "product root has no final component",
        )
    })?;
    let parent = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|e| io_fail("MATERIALIZER_SOURCE_ROOT", root, e))?;
    let metadata = parent
        .symlink_metadata(name)
        .map_err(|e| fail("MATERIALIZER_SOURCE_ROOT", "product-root", e.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(fail(
            "MATERIALIZER_SOURCE_ROOT",
            "product-root",
            "product root must be a regular non-symlink directory",
        ));
    }
    open_dir_nofollow(&parent, name)
        .map_err(|e| fail("MATERIALIZER_SOURCE_ROOT", "product-root", e.to_string()))
}

fn open_dir_nofollow(parent: &Dir, name: impl AsRef<Path>) -> io::Result<Dir> {
    let parent = parent.try_clone()?.into_std_file();
    Ok(Dir::from_std_file(cap_primitives::fs::open_dir_nofollow(
        &parent,
        name.as_ref(),
    )?))
}

fn open_authored_parent(root: &Dir, relative: &str) -> Result<(Dir, String), MaterializationError> {
    let parts = relative.split('/').collect::<Vec<_>>();
    let (name, parents) = parts
        .split_last()
        .ok_or_else(|| fail("MATERIALIZER_SOURCE_PATH", relative, "empty source path"))?;
    let mut current = root
        .try_clone()
        .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
    for parent in parents {
        let metadata = current
            .symlink_metadata(parent)
            .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(fail(
                "MATERIALIZER_SOURCE_SYMLINK",
                relative,
                "source path contains a symlink component",
            ));
        }
        if !metadata.is_dir() {
            return Err(fail(
                "MATERIALIZER_SOURCE_KIND",
                relative,
                "source path parent is not a directory",
            ));
        }
        current = open_dir_nofollow(&current, parent)
            .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
    }
    Ok((current, (*name).to_owned()))
}

fn open_authored_file(
    root: &Dir,
    relative: &str,
) -> Result<cap_std::fs::File, MaterializationError> {
    let (parent, name) = open_authored_parent(root, relative)?;
    let mut options = OpenOptions::new();
    options.read(true);
    options._cap_fs_ext_follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(fail(
            "MATERIALIZER_SOURCE_KIND",
            relative,
            "source closure entries must be regular non-symlink files",
        ));
    }
    Ok(file)
}

fn authored_file_exists(root: &Dir, relative: &str) -> Result<bool, MaterializationError> {
    match open_authored_file(root, relative) {
        Ok(_) => Ok(true),
        Err(error) if error.diagnostic().code() == "MATERIALIZER_SOURCE_READ" => Ok(false),
        Err(error) => Err(error),
    }
}

fn read_authored_file(root: &Dir, relative: &str) -> Result<Vec<u8>, MaterializationError> {
    let mut file = open_authored_file(root, relative)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| fail("MATERIALIZER_SOURCE_READ", relative, e.to_string()))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(fail(
            "MATERIALIZER_SOURCE_BOUNDS",
            relative,
            "source file exceeds bound",
        ));
    }
    Ok(bytes)
}

fn normalize_relative(path: &Path) -> Result<String, MaterializationError> {
    let mut parts = Vec::new();
    for part in path.components() {
        use std::path::Component;
        match part {
            Component::Normal(v) => parts.push(v.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(fail(
                        "MATERIALIZER_IMPORT_ESCAPE",
                        "import",
                        "relative import escapes its authored lane",
                    ));
                }
            }
            _ => {
                return Err(fail(
                    "MATERIALIZER_IMPORT_ESCAPE",
                    "import",
                    "non-relative import path",
                ))
            }
        }
    }
    let value = parts.join("/");
    validate_relative(&value, "import")?;
    Ok(value)
}

fn run_node(
    node: &Path,
    script: &Path,
    args: &[&OsStr],
    cwd: &Path,
    limits: MaterializationLimits,
) -> Result<Output, MaterializationError> {
    let mut command = Command::new(node);
    // Node's permission model denies network, child-process, workers, env
    // writes, and every filesystem path except this fresh temporary closure.
    command
        .arg("--permission")
        .arg(format!("--allow-fs-read={}", cwd.display()))
        .arg(format!("--allow-fs-write={}", cwd.display()));
    command
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_child(command, script, limits)
}

fn run_trusted_node(
    node: &Path,
    script: &Path,
    args: &[&OsStr],
    cwd: &Path,
    limits: MaterializationLimits,
) -> Result<Output, MaterializationError> {
    let mut command = Command::new(node);
    command
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_child(command, script, limits)
}

fn run_child(
    mut command: Command,
    script: &Path,
    limits: MaterializationLimits,
) -> Result<Output, MaterializationError> {
    let mut child = command.spawn().map_err(|e| {
        fail(
            "MATERIALIZER_TOOL_START",
            script.to_string_lossy().as_ref(),
            e.to_string(),
        )
    })?;
    let mut stdout = child.stdout.take().expect("stdout pipe configured");
    let mut stderr = child.stderr.take().expect("stderr pipe configured");
    let (overflow_sender, overflow_receiver) = mpsc::channel();
    let stdout_sender = overflow_sender.clone();
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout
            .by_ref()
            .take((MAX_TOOL_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes);
        if bytes.len() > MAX_TOOL_OUTPUT_BYTES {
            let _ = stdout_sender.send(());
        }
        bytes
    });
    let stderr_sender = overflow_sender;
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr
            .by_ref()
            .take((MAX_TOOL_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes);
        if bytes.len() > MAX_TOOL_OUTPUT_BYTES {
            let _ = stderr_sender.send(());
        }
        bytes
    });
    let started = Instant::now();
    loop {
        if overflow_receiver.try_recv().is_ok() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(fail(
                "MATERIALIZER_TOOL_OUTPUT_BOUNDS",
                script.to_string_lossy().as_ref(),
                "build tool stdout or stderr exceeded bound",
            ));
        }
        if child
            .try_wait()
            .map_err(|e| {
                fail(
                    "MATERIALIZER_TOOL_WAIT",
                    script.to_string_lossy().as_ref(),
                    e.to_string(),
                )
            })?
            .is_some()
        {
            let status = child.wait().map_err(|e| {
                fail(
                    "MATERIALIZER_TOOL_WAIT",
                    script.to_string_lossy().as_ref(),
                    e.to_string(),
                )
            })?;
            let stdout = stdout_reader.join().unwrap_or_default();
            let stderr = stderr_reader.join().unwrap_or_default();
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        if started.elapsed() > limits.timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(fail(
                "MATERIALIZER_TOOL_TIMEOUT",
                script.to_string_lossy().as_ref(),
                "build tool exceeded its fixed timeout",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Vite is an Engine-owned build tool, not an authored-code sandbox. It only
/// receives the already scanned/copied scratch closure plus a fixed generator
/// config with no plugin input; its toolchain path is supplied explicitly.
fn run_trusted_vite(
    node: &Path,
    script: &Path,
    args: &[&OsStr],
    cwd: &Path,
    limits: MaterializationLimits,
) -> Result<Output, MaterializationError> {
    let mut command = Command::new(node);
    command
        .arg(script)
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    run_child(command, script, limits)
}

fn verify_node_permission_support(node: &Path, cwd: &Path) -> Result<(), MaterializationError> {
    let status = Command::new(node)
        .arg("--permission")
        .arg(format!("--allow-fs-read={}", cwd.display()))
        .arg("-e")
        .arg("process.exit(0)")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| fail("MATERIALIZER_PERMISSION_SUPPORT", "node", e.to_string()))?;
    if !status.success() {
        return Err(fail(
            "MATERIALIZER_PERMISSION_SUPPORT",
            "node",
            "Node permission mode is unavailable",
        ));
    }
    Ok(())
}
fn check_output(code: &str, path: &str, output: Output) -> Result<Output, MaterializationError> {
    if output.stdout.len() > MAX_TOOL_OUTPUT_BYTES || output.stderr.len() > MAX_TOOL_OUTPUT_BYTES {
        return Err(fail(
            "MATERIALIZER_TOOL_OUTPUT_BOUNDS",
            path,
            "build tool stdout or stderr exceeded bound",
        ));
    }
    if !output.status.success() {
        return Err(fail(code, path, String::from_utf8_lossy(&output.stderr)));
    }
    Ok(output)
}

fn collect_output(
    directory: &Path,
    prefix: &str,
) -> Result<Vec<PublicationFile>, MaterializationError> {
    let mut files = Vec::new();
    collect_files(directory, directory, prefix, &mut files)?;
    if files.is_empty() {
        return Err(fail(
            "MATERIALIZER_UI_EMPTY",
            prefix,
            "Vite did not produce UI assets",
        ));
    }
    Ok(files)
}
fn collect_files(
    root: &Path,
    current: &Path,
    prefix: &str,
    files: &mut Vec<PublicationFile>,
) -> Result<(), MaterializationError> {
    for entry in fs::read_dir(current).map_err(|e| io_fail("MATERIALIZER_UI_READ", current, e))? {
        let entry = entry.map_err(|e| io_fail("MATERIALIZER_UI_READ", current, e))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|e| io_fail("MATERIALIZER_UI_READ", &path, e))?;
        if kind.is_symlink() {
            return Err(fail(
                "MATERIALIZER_UI_SYMLINK",
                path.to_string_lossy().as_ref(),
                "generated UI assets must not be symlinks",
            ));
        }
        if kind.is_dir() {
            collect_files(root, &path, prefix, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| {
                    fail(
                        "MATERIALIZER_UI_PATH",
                        path.to_string_lossy().as_ref(),
                        "generated output escaped root",
                    )
                })?
                .to_string_lossy()
                .replace('\\', "/");
            if relative.ends_with(".map") {
                continue;
            }
            let bytes = fs::read(&path).map_err(|e| io_fail("MATERIALIZER_UI_READ", &path, e))?;
            files.push(
                PublicationFile::new(format!("{prefix}/{relative}"), bytes)
                    .map_err(|e| fail("MATERIALIZER_UI_BOUNDS", &relative, e.to_string()))?,
            );
        }
    }
    Ok(())
}
fn reject_runtime_leaks(
    files: &[PublicationFile],
    temp: &Path,
) -> Result<(), MaterializationError> {
    let needle = temp.to_string_lossy();
    for file in files {
        let value = String::from_utf8_lossy(file.bytes());
        if value.contains("node_modules")
            || value.contains("sourceMappingURL")
            || value.contains(needle.as_ref())
            || value.contains("from '@rusty-engine/")
            || value.contains("from \"@rusty-engine/")
        {
            return Err(fail(
                "MATERIALIZER_RUNTIME_LEAK",
                file.relative_path().as_str(),
                "runtime asset retained a build-time path, source map, or bare Engine import",
            ));
        }
    }
    Ok(())
}

fn validate_emitted_modules(
    files: &[PublicationFile],
    module_root: &Path,
    scanner: &Path,
    node: &Path,
    typescript: &Path,
    cwd: &Path,
    limits: MaterializationLimits,
) -> Result<(), MaterializationError> {
    let names = files
        .iter()
        .map(|file| file.relative_path().as_str())
        .collect::<BTreeSet<_>>();
    for file in files {
        let relative = file.relative_path().as_str();
        let path = module_root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| io_fail("MATERIALIZER_EMITTED_MODULE", parent, e))?;
        }
        fs::write(&path, file.bytes())
            .map_err(|e| io_fail("MATERIALIZER_EMITTED_MODULE", &path, e))?;
    }
    for file in files
        .iter()
        .filter(|file| file.relative_path().as_str().ends_with(".js"))
    {
        let relative = file.relative_path().as_str();
        let output = run_node(
            node,
            scanner,
            &[
                typescript.as_os_str(),
                module_root.join(relative).as_os_str(),
            ],
            cwd,
            limits,
        )?;
        let output = check_output("MATERIALIZER_EMITTED_MODULE", relative, output)?;
        let imports: Imports = serde_json::from_slice(&output.stdout)
            .map_err(|e| fail("MATERIALIZER_EMITTED_MODULE", relative, e.to_string()))?;
        if imports.dynamic {
            return Err(fail(
                "MATERIALIZER_RUNTIME_LEAK",
                relative,
                "emitted JavaScript must not contain dynamic import or require",
            ));
        }
        for specifier in imports.specifiers {
            if !specifier.starts_with('.') || specifier.starts_with("//") {
                return Err(fail(
                    "MATERIALIZER_RUNTIME_LEAK",
                    relative,
                    "emitted JavaScript must not contain bare or absolute module specifiers",
                ));
            }
            let joined = Path::new(relative)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(specifier);
            let resolved = normalize_relative(&joined)?;
            if !names.contains(resolved.as_str()) {
                return Err(fail(
                    "MATERIALIZER_RUNTIME_LEAK",
                    relative,
                    "emitted JavaScript module specifier escaped the generated closure",
                ));
            }
        }
    }
    Ok(())
}
fn vite_config(entry: &str, out: &Path) -> String {
    format!("export default {{ root: 'src', build: {{ outDir: {}, emptyOutDir: true, sourcemap: false, rollupOptions: {{ input: {}, preserveEntrySignatures: 'strict', output: {{ format: 'es', entryFileNames: 'main.js', chunkFileNames: 'chunks/[name]-[hash].js', assetFileNames: 'assets/[name]-[hash][extname]' }} }} }} }};\n", serde_json::to_string(&out.to_string_lossy()).unwrap_or_else(|_| "\"out\"".into()), serde_json::to_string(entry).unwrap_or_else(|_| "\"ui.ts\"".into()))
}
fn runtime_vite_config(entry: &Path, out: &Path) -> String {
    format!(
        "export default {{ build: {{ outDir: {}, emptyOutDir: true, sourcemap: false, rollupOptions: {{ input: {}, preserveEntrySignatures: 'strict', output: {{ format: 'es', entryFileNames: 'runtime.js', inlineDynamicImports: true }} }} }} }};\n",
        serde_json::to_string(&out.to_string_lossy()).unwrap_or_else(|_| "\"out/runtime\"".into()),
        serde_json::to_string(&entry.to_string_lossy()).unwrap_or_else(|_| "\"runtime-entry.ts\"".into()),
    )
}
fn lifecycle_name(mode: LifecycleMode) -> &'static str {
    match mode {
        LifecycleMode::Realtime => "realtime",
        LifecycleMode::Demand => "demand",
        LifecycleMode::External => "external",
    }
}
fn validate_package(value: &str) -> Result<(), MaterializationError> {
    if !value.starts_with("@rusty-engine/")
        || value.len() > 128
        || value.contains("..")
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(fail(
            "MATERIALIZER_ENGINE_PACKAGE",
            value,
            "Engine package identity is not admitted",
        ));
    }
    Ok(())
}
fn validate_relative(value: &str, path: &str) -> Result<(), MaterializationError> {
    ProductPath::parse(value.to_owned())
        .map_err(|e| fail("MATERIALIZER_PATH", path, e.to_string()))?;
    Ok(())
}
fn io_fail(code: &str, path: &Path, error: io::Error) -> MaterializationError {
    fail(code, path.to_string_lossy().as_ref(), error.to_string())
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bound(mut text: String) -> String {
    const MAX: usize = 2048;
    if text.len() > MAX {
        let mut end = MAX;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

const SCANNER: &str = r#"import { pathToFileURL } from 'node:url';
const [typescriptPath, file, lane] = process.argv.slice(2);
const imported = await import(pathToFileURL(typescriptPath).href);
const ts = imported.default ?? imported;
const source = ts.createSourceFile(
  file,
  await (await import('node:fs/promises')).readFile(file, 'utf8'),
  ts.ScriptTarget.ESNext,
  true,
);
const specifiers = [];
const violations = [];
let dynamic = false;

function textLiteral(node) {
  return ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) ? node.text : undefined;
}
function memberName(expression) {
  if (ts.isPropertyAccessExpression(expression)) return expression.name.text;
  if (ts.isElementAccessExpression(expression)) return expression.argumentExpression ? textLiteral(expression.argumentExpression) : undefined;
  return undefined;
}
function expressionName(expression) {
  if (ts.isIdentifier(expression)) return expression.text;
  return memberName(expression);
}
function report(node, code, message) {
  const position = source.getLineAndCharacterOfPosition(node.getStart(source));
  violations.push({ code, message, line: position.line + 1, column: position.character + 1 });
}
function hostBootstrapSpecifier(specifier) {
  return specifier.startsWith('node:')
    || ['vite', 'webpack', 'esbuild', 'express', 'fastify', 'http', 'https', 'net'].includes(specifier);
}
function rendererImplementationSpecifier(specifier) {
  return specifier.startsWith('@rusty-engine/renderer-')
    || specifier.includes('/private/')
    || specifier.endsWith('/private');
}
function importedName(importDeclaration, name) {
  const bindings = importDeclaration.importClause?.namedBindings;
  if (bindings && ts.isNamedImports(bindings)) {
    return bindings.elements.some((entry) => (entry.propertyName ?? entry.name).text === name);
  }
  return Boolean(bindings && ts.isNamespaceImport(bindings));
}
function directInputEvent(value) {
  return ['keydown', 'keyup', 'keypress', 'gamepadconnected', 'gamepaddisconnected', 'controllerconnected', 'controllerdisconnected'].includes(value);
}
function directInputProperty(value) {
  return ['onkeydown', 'onkeyup', 'onkeypress', 'ongamepadconnected', 'ongamepaddisconnected', 'oncontrollerconnected', 'oncontrollerdisconnected'].includes(value);
}
function hostBootstrapCall(name) {
  return ['mountRustyApplication', 'mountProductBrowserHost', 'productBrowserBundleDescriptor', 'createServer', 'listen', 'serve'].includes(name);
}
function retainedExportCallback(node) {
  const parent = node.parent;
  if (!(ts.isPropertyAssignment(parent) && parent.initializer === node)
      && !(ts.isArrayLiteralExpression(parent) && parent.elements.includes(node))) return false;
  for (let current = parent.parent; current && !ts.isSourceFile(current); current = current.parent) {
    if (ts.isExportAssignment(current)) return true;
    if (ts.isFunctionLike(current)) return false;
  }
  return false;
}
function visit(node) {
  if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
    const specifier = textLiteral(node.moduleSpecifier);
    if (specifier !== undefined) {
      specifiers.push(specifier);
      if (rendererImplementationSpecifier(specifier)) {
        report(node.moduleSpecifier, 'MATERIALIZER_RENDERER_IMPORT', 'renderer implementation or private module imports are not admitted; render through the Engine-owned application host');
      } else if (hostBootstrapSpecifier(specifier)) {
        report(node.moduleSpecifier, 'MATERIALIZER_HOST_BOOTSTRAP', 'downstream source must not bootstrap a dev server or host; use generated Product Assembly and `rusty dev`');
      }
    }
    if (ts.isImportDeclaration(node) && node.moduleSpecifier && textLiteral(node.moduleSpecifier) === '@rusty-engine/application-host' && importedName(node, 'mountRustyApplication')) {
      report(node, 'MATERIALIZER_HOST_BOOTSTRAP', 'downstream UI may mount presentation only; the Engine-owned bundle mounts the application host');
    }
    if (ts.isImportDeclaration(node) && node.moduleSpecifier && textLiteral(node.moduleSpecifier) === '@rusty-engine/product-browser-host' && (importedName(node, 'mountProductBrowserHost') || importedName(node, 'productBrowserBundleDescriptor'))) {
      report(node, 'MATERIALIZER_HOST_BOOTSTRAP', 'downstream UI must not bootstrap the browser host or alter its bundle; use generated Product Assembly');
    }
  }
  if (ts.isCallExpression(node)) {
    const name = expressionName(node.expression);
    if (node.expression.kind === ts.SyntaxKind.ImportKeyword || name === 'require') dynamic = true;
    if (lane === 'ui' && name === 'requestAnimationFrame') {
      report(node, 'MATERIALIZER_UI_RENDER_LOOP', 'downstream UI must not run a render loop; subscribe to bounded presentation or UI projection instead');
    }
    if (lane === 'ui' && name === 'addEventListener') {
      const eventName = node.arguments.length > 0 ? textLiteral(node.arguments[0]) : undefined;
      if (eventName !== undefined && directInputEvent(eventName)) {
        report(node, 'MATERIALIZER_UI_INPUT_LISTENER', 'downstream UI must not capture keyboard or controller gameplay input; declare input mappings in Runtime Composition');
      }
    }
    if (lane === 'ui' && (name === 'createElement' || name === 'createElementNS')) {
      const canvasArgument = node.arguments.length === 0 ? undefined : textLiteral(node.arguments[node.arguments.length - 1]);
      if (canvasArgument?.toLowerCase() === 'canvas') {
        report(node, 'MATERIALIZER_UI_CANVAS', 'downstream UI must not create a canvas; the Engine-owned application host owns the sole renderer canvas');
      }
    }
    if (name !== undefined && hostBootstrapCall(name)) {
      report(node, 'MATERIALIZER_HOST_BOOTSTRAP', 'downstream source must not start or configure a host; use the generated Engine-owned host');
    }
    if (lane === 'rules' && ['eval', 'Function', 'setTimeout', 'setInterval', 'queueMicrotask'].includes(name ?? '')) {
      report(node, 'MATERIALIZER_COMPOSITION_DYNAMIC', 'Runtime Composition must be serializable authored data, not dynamically evaluated behavior');
    }
  }
  if (lane === 'ui' && ts.isNewExpression(node) && expressionName(node.expression) === 'OffscreenCanvas') {
    report(node, 'MATERIALIZER_UI_CANVAS', 'downstream UI must not create a canvas; the Engine-owned application host owns the sole renderer canvas');
  }
  if (lane === 'runtime' && ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'globalThis') {
    report(node, 'MATERIALIZER_RUNTIME_GLOBAL', 'runtime source must default-export its ABI object; the Engine-generated wrapper owns global installation');
  }
  if (lane === 'ui' && ts.isBinaryExpression(node) && ts.isAssignmentOperator(node.operatorToken.kind) && directInputProperty(memberName(node.left) ?? '')) {
    report(node, 'MATERIALIZER_UI_INPUT_LISTENER', 'downstream UI must not capture keyboard or controller gameplay input; declare input mappings in Runtime Composition');
  }
  if (lane === 'rules' && (ts.isArrowFunction(node) || ts.isFunctionExpression(node)) && retainedExportCallback(node)) {
    report(node, 'MATERIALIZER_COMPOSITION_CALLBACK', 'Runtime Composition must not retain callbacks; express behavior with admitted DSL declarations and Product Kernel bindings');
  }
  ts.forEachChild(node, visit);
}
visit(source);
console.log(JSON.stringify({ specifiers, dynamic, violations }));
"#;
const RULES_RUNNER: &str = r#"import { pathToFileURL } from 'node:url';
const [product, joinedEntries, output] = process.argv.slice(2);
const entries = joinedEntries.split('\n').filter(Boolean); if (entries.length === 0) throw new Error('no rules entrypoints');
const authoring = await import('@rusty-engine/runtime-composition-authoring');
let base;
for (let index = 0; index < entries.length; index += 1) { const mod = await import(pathToFileURL(entries[index]).href); if (!('default' in mod)) throw new Error(`rules entry ${entries[index]} must have a default export`); if (index === 0) base = authoring.authorRuntimeComposition(mod.default).composition; else base = authoring.appendComposition(base, mod.default); }
if (base.product !== product) throw new Error('RuntimeCompositionDraft product does not match rusty.toml');
await (await import('node:fs/promises')).writeFile(output, JSON.stringify(base));
"#;

const TYPECHECKER: &str = r#"import { pathToFileURL } from 'node:url';
const [typescriptPath, laneRoot, lane, harness] = process.argv.slice(2);
const imported = await import(pathToFileURL(typescriptPath).href); const ts = imported.default ?? imported;
const fs = await import('node:fs/promises');
async function files(root) { const output = []; for (const entry of await fs.readdir(root, { withFileTypes: true })) { const path = `${root}/${entry.name}`; if (entry.isDirectory()) output.push(...await files(path)); else if (/\.(ts|tsx|mts)$/.test(entry.name)) output.push(path); } return output; }
const roots = await files(laneRoot); if (harness) roots.push(harness); if (roots.length === 0) throw new Error(`${lane} lane has no TypeScript sources`);
const options = { strict: true, noEmit: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.NodeNext, moduleResolution: ts.ModuleResolutionKind.NodeNext, types: [], lib: lane === 'ui' ? ['lib.es2022.d.ts', 'lib.dom.d.ts'] : ['lib.es2022.d.ts'] };
const program = ts.createProgram({ rootNames: roots, options }); const diagnostics = ts.getPreEmitDiagnostics(program);
if (diagnostics.length) throw new Error(ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCanonicalFileName: value => value, getCurrentDirectory: () => laneRoot, getNewLine: () => '\n' }));
"#;

const RULES_COMPILER: &str = r#"import { pathToFileURL } from 'node:url';
const [typescriptPath, sourceRoot, outputRoot] = process.argv.slice(2);
const imported = await import(pathToFileURL(typescriptPath).href); const ts = imported.default ?? imported; const fs = await import('node:fs/promises');
async function compile(current, relative = '') { for (const entry of await fs.readdir(current, { withFileTypes: true })) { const input = `${current}/${entry.name}`; const output = relative ? `${relative}/${entry.name}` : entry.name; if (entry.isDirectory()) await compile(input, output); else if (/\.(ts|tsx|mts)$/.test(entry.name)) { const code = await fs.readFile(input, 'utf8'); const emitted = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.NodeNext, sourceMap: false, inlineSourceMap: false } }).outputText; const target = `${outputRoot}/${output.replace(/\.(ts|tsx|mts)$/, '.js')}`; await fs.mkdir(target.slice(0, target.lastIndexOf('/')), { recursive: true }); await fs.writeFile(target, emitted); } } }
await compile(sourceRoot);
"#;

const BUNDLE_RUNNER: &str = r#"const [lifecycleMode, projectionStream, projectionContract, output] = process.argv.slice(2);
const host = await import('@rusty-engine/product-browser-host');
const fs = await import('node:fs/promises');
const engineHostModule = await fs.readFile(new URL('./node_modules/@rusty-engine/product-browser-host/product-browser-host.js', import.meta.url), 'utf8');
if ((projectionStream === '') !== (projectionContract === '')) throw new Error('UI projection stream and contract must be supplied together');
const descriptor = host.productBrowserBundleDescriptor({
  engineHostModule,
  uiModule: './ui/main.js',
  runtimeAdapterModule: './runtime-adapter.js',
  lifecycleMode,
  uiProjection: projectionStream === '' ? undefined : { expectedStream: projectionStream, expectedContract: projectionContract },
});
await fs.writeFile(output, JSON.stringify(descriptor));
"#;

#[derive(Deserialize)]
struct BundleDescriptor {
    artifact: String,
    files: Vec<BundleFile>,
}
#[derive(Deserialize)]
struct BundleFile {
    name: String,
    content: String,
    #[serde(rename = "utf8Bytes")]
    utf8_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use product_model::decode_product_manifest;
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn rejects_engine_asset_paths_that_could_escape_the_temp_closure() {
        let error = EngineAsset::new("@rusty-engine/product-browser-host", "../host.js", vec![])
            .expect_err("path escape must fail");
        assert_eq!(error.diagnostic().code(), "MATERIALIZER_PATH");
    }

    #[test]
    fn normalizes_nested_relative_imports_without_crossing_a_lane() {
        assert_eq!(
            normalize_relative(Path::new("rules/nested/../entry")).expect("nested import"),
            "rules/entry"
        );
        assert!(normalize_relative(Path::new("../../entry")).is_err());
    }

    #[test]
    fn browser_descriptor_requires_the_current_engine_artifact_identity() {
        let descriptor: BundleDescriptor =
            serde_json::from_str(r#"{"artifact":"other","files":[]}"#).expect("decode descriptor");
        assert_ne!(descriptor.artifact, "rusty.product.bundle");
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn materializes_a_rules_draft_ordered_fragment_and_relocatable_ui_bundle() {
        let fixture = Fixture::new("success", &[
            ("rules/main.ts", "import type { RuntimeCompositionDraft } from '@rusty-engine/runtime-composition-authoring';\nimport { base } from './base.js';\nexport default base satisfies RuntimeCompositionDraft;\n"),
            ("rules/base.ts", "export const base = { product: 'rusty.test', capabilities: [] };\n"),
            ("rules/extra.ts", "import { fragment, gameplayDefinition } from '@rusty-engine/runtime-composition-authoring';\nexport default fragment({ gameplayDefinitions: [gameplayDefinition('later', { order: 2 })] });\n"),
            ("runtime/main.ts", "import { next } from './state.js';\nexport default { initialize: ({ facts }: { facts: { start: number } }) => ({ count: facts.start }), turn: ({ state, input }: { state: { count: number }, input: { amount: number } }) => next(state, input.amount), project: ({ state }: { state: { count: number } }) => ({ count: state.count }) };\n"),
            ("runtime/state.ts", "export const next = (state: { count: number }, amount: number): { count: number } => ({ count: state.count + amount });\n"),
            ("ui/main.ts", "export function mountProductUi(_root: Element, _context: unknown): void {}\n"),
        ]);
        let first = materialize_product(
            &fixture.root,
            &fixture.manifest,
            &fixture.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect("first materialization");
        fs::create_dir_all(fixture.root.join("generated")).expect("generated");
        fs::remove_dir_all(fixture.root.join("generated")).expect("delete generated");
        let second = materialize_product(
            &fixture.root,
            &fixture.manifest,
            &fixture.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect("second materialization");
        let plain = materialize_product(
            &fixture.root,
            &fixture.manifest,
            &fixture.assets,
            &plain_toolchain(),
            MaterializationLimits::default(),
        )
        .expect("plain temporary parent materialization");
        assert_eq!(first.compiled_composition(), second.compiled_composition());
        assert_eq!(first.compiled_composition(), plain.compiled_composition());
        assert_eq!(
            file_bytes(first.browser_bundle_files()),
            file_bytes(second.browser_bundle_files())
        );
        let text =
            String::from_utf8(first.compiled_composition().to_vec()).expect("composition text");
        assert!(text.contains("\"later\""));
        let runtime_program = first.runtime_program().expect("runtime program");
        let runtime_text = String::from_utf8_lossy(runtime_program);
        assert!(runtime_text.contains("__rustyEngineRuntime"));
        assert!(!runtime_text.contains("RustyEngineRuntimeBundle"));
        assert!(!runtime_text.contains("import "));
        assert!(!runtime_text.contains(" from "));
        let runtime_program_path = fixture.root.join("runtime-program.js");
        fs::write(&runtime_program_path, runtime_program).expect("runtime program");
        let runtime_check = fixture.root.join("runtime-program-check.mjs");
        fs::write(
            &runtime_check,
            "const before = new Set(Object.getOwnPropertyNames(globalThis));\nawait import('./runtime-program.js');\nconst added = Object.getOwnPropertyNames(globalThis).filter((name) => !before.has(name));\nif (added.length !== 1 || added[0] !== '__rustyEngineRuntime') throw new Error(`unexpected runtime globals: ${added.join(',')}`);\nconst runtime = globalThis.__rustyEngineRuntime;\nif (Object.getOwnPropertyNames(runtime).sort().join(',') !== 'initialize,project,turn') throw new Error('runtime export names are not exact');\n",
        )
        .expect("runtime check");
        let runtime_check_output = run_node(
            &node_from_path(),
            &runtime_check,
            &[],
            &fixture.root,
            MaterializationLimits::default(),
        )
        .expect("start runtime check");
        check_output(
            "MATERIALIZER_RUNTIME_BUNDLE",
            "runtime",
            runtime_check_output,
        )
        .expect("runtime must install only its fixed global");
        let files = file_bytes(first.browser_bundle_files());
        for required in [
            "index.html",
            "main.js",
            "bridge.js",
            "runtime-adapter.js",
            "engine/product-browser-host.js",
            "ui/main.js",
        ] {
            assert!(files.contains_key(required), "missing {required}");
        }
        assert!(
            String::from_utf8_lossy(files.get("ui/main.js").expect("UI entry"))
                .contains("mountProductUi"),
            "bundled UI entry must retain its required named export"
        );
        for (path, bytes) in files {
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains(fixture.root.to_string_lossy().as_ref()),
                "temporary/root path leaked in {path}"
            );
            assert!(
                !text.contains("node_modules"),
                "node module path leaked in {path}"
            );
        }
        fixture.cleanup();
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn rejects_rules_dom_and_ui_without_the_required_mount_export() {
        let rules_dom = Fixture::new("rules-dom", &[
            ("rules/main.ts", "const node = document.createElement('div');\nexport default { product: 'rusty.test', capabilities: [] };\nvoid node;\n"),
            ("ui/main.ts", "export function mountProductUi(): void {}\n"),
        ]);
        let error = materialize_product(
            &rules_dom.root,
            &rules_dom.manifest,
            &rules_dom.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect_err("rules DOM must fail");
        assert_eq!(error.diagnostic().code(), "MATERIALIZER_RULES_TYPECHECK");
        rules_dom.cleanup();
        let missing_mount = Fixture::new(
            "missing-mount",
            &[
                (
                    "rules/main.ts",
                    "const authored = (value: string): string => value; export default { product: authored('rusty.test'), capabilities: [] };\n",
                ),
                ("ui/main.ts", "export const notTheMount = 1;\n"),
            ],
        );
        let error = materialize_product(
            &missing_mount.root,
            &missing_mount.manifest,
            &missing_mount.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect_err("missing mount must fail");
        assert_eq!(error.diagnostic().code(), "MATERIALIZER_UI_TYPECHECK");
        missing_mount.cleanup();
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn rejects_unbounded_or_cross_lane_import_forms_before_any_build() {
        for (label, rules, expected) in [
            ("dynamic", "export default import('./later.js');\n", "MATERIALIZER_DYNAMIC_IMPORT"),
            ("require", "const value = require('./later.js'); export default value;\n", "MATERIALIZER_DYNAMIC_IMPORT"),
            ("bare", "import 'unadmitted-package'; export default { product: 'rusty.test', capabilities: [] };\n", "MATERIALIZER_BARE_IMPORT"),
            ("cross", "import '../ui/secret.js'; export default { product: 'rusty.test', capabilities: [] };\n", "MATERIALIZER_IMPORT_ESCAPE"),
        ] {
            let fixture = Fixture::new(label, &[("rules/main.ts", rules), ("ui/main.ts", "export function mountProductUi(): void {}\n"), ("ui/secret.ts", "export const secret = 1;\n")]);
            let error = materialize_product(&fixture.root, &fixture.manifest, &fixture.assets, &toolchain(), MaterializationLimits::default()).expect_err(label);
            assert_eq!(error.diagnostic().code(), expected, "{label}");
            fixture.cleanup();
        }
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn rejects_authored_host_authority_with_source_path_diagnostics() {
        let valid_rules = "export default { product: 'rusty.test', capabilities: [] };\n";
        let valid_ui =
            "export function mountProductUi(_root: Element, _context: unknown): void {}\n";
        for (label, path, source, expected) in [
            (
                "canvas",
                "ui/main.ts",
                "export function mountProductUi(root: Element, _context: unknown): void { root.append(document.createElement('canvas')); }\n",
                "MATERIALIZER_UI_CANVAS",
            ),
            (
                "mixed-case-canvas",
                "ui/main.ts",
                "export function mountProductUi(root: Element, _context: unknown): void { root.append(document.createElement('Canvas')); }\n",
                "MATERIALIZER_UI_CANVAS",
            ),
            (
                "offscreen-canvas",
                "ui/main.ts",
                "export function mountProductUi(_root: Element, _context: unknown): void { void new OffscreenCanvas(1, 1); }\n",
                "MATERIALIZER_UI_CANVAS",
            ),
            (
                "render-loop",
                "ui/main.ts",
                "export function mountProductUi(_root: Element, _context: unknown): void { requestAnimationFrame(() => {}); }\n",
                "MATERIALIZER_UI_RENDER_LOOP",
            ),
            (
                "keyboard-listener",
                "ui/main.ts",
                "export function mountProductUi(_root: Element, _context: unknown): void { document.addEventListener('keydown', () => {}); }\n",
                "MATERIALIZER_UI_INPUT_LISTENER",
            ),
            (
                "controller-property",
                "ui/main.ts",
                "export function mountProductUi(_root: Element, _context: unknown): void { window.ongamepadconnected = () => {}; }\n",
                "MATERIALIZER_UI_INPUT_LISTENER",
            ),
            (
                "renderer-import",
                "ui/main.ts",
                "import '@rusty-engine/renderer-three'; export function mountProductUi(_root: Element, _context: unknown): void {}\n",
                "MATERIALIZER_RENDERER_IMPORT",
            ),
            (
                "private-import",
                "ui/main.ts",
                "import '@rusty-engine/application-host/private/bridge'; export function mountProductUi(_root: Element, _context: unknown): void {}\n",
                "MATERIALIZER_RENDERER_IMPORT",
            ),
            (
                "host-bootstrap",
                "ui/main.ts",
                "import { mountRustyApplication } from '@rusty-engine/application-host'; export function mountProductUi(_root: Element, _context: unknown): void { void mountRustyApplication; }\n",
                "MATERIALIZER_HOST_BOOTSTRAP",
            ),
            (
                "node-server",
                "ui/main.ts",
                "import { createServer } from 'node:http'; export function mountProductUi(_root: Element, _context: unknown): void { void createServer; }\n",
                "MATERIALIZER_HOST_BOOTSTRAP",
            ),
            (
                "composition-callback",
                "rules/main.ts",
                "export default { product: 'rusty.test', capabilities: [], callback: () => 1 };\n",
                "MATERIALIZER_COMPOSITION_CALLBACK",
            ),
            (
                "composition-dynamic",
                "rules/main.ts",
                "setInterval(() => {}, 1); export default { product: 'rusty.test', capabilities: [] };\n",
                "MATERIALIZER_COMPOSITION_DYNAMIC",
            ),
        ] {
            let fixture = Fixture::new(
                label,
                &[
                    (
                        "rules/main.ts",
                        if path == "rules/main.ts" {
                            source
                        } else {
                            valid_rules
                        },
                    ),
                    (
                        "ui/main.ts",
                        if path == "ui/main.ts" { source } else { valid_ui },
                    ),
                ],
            );
            let error = materialize_product(
                &fixture.root,
                &fixture.manifest,
                &fixture.assets,
                &toolchain(),
                MaterializationLimits::default(),
            )
            .expect_err(label);
            assert_eq!(error.diagnostic().code(), expected, "{label}");
            assert_eq!(error.diagnostic().path(), path, "{label}");
            assert!(
                error.diagnostic().message().contains("line "),
                "{label}: {}",
                error.diagnostic().message()
            );
            fixture.cleanup();
        }
    }

    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn permits_ordinary_dom_ui_click_intents_and_presentation() {
        let fixture = Fixture::new(
            "ui-click-intent",
            &[
                (
                    "rules/main.ts",
                    "export default { product: 'rusty.test', capabilities: [] };\n",
                ),
                (
                    "ui/main.ts",
                    "import type { RustyApplicationUiContext } from '@rusty-engine/application-host';\nexport function mountProductUi(root: HTMLElement, context: RustyApplicationUiContext): void { const button = document.createElement('button'); button.textContent = 'continue'; button.addEventListener('click', () => context.intents?.claim('continue', { kind: 'digital', active: true })); root.append(button); }\n",
                ),
            ],
        );
        materialize_product(
            &fixture.root,
            &fixture.manifest,
            &fixture.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect("ordinary DOM UI click intent and presentation must remain admitted");
        fixture.cleanup();
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
    fn rejects_intermediate_symlink_in_an_authored_closure() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new("symlink", &[
            ("rules/main.ts", "import './nested/child.js'; export default { product: 'rusty.test', capabilities: [] };\n"),
            ("ui/main.ts", "export function mountProductUi(): void {}\n"),
        ]);
        fs::create_dir_all(fixture.root.join("outside")).expect("outside");
        fs::write(
            fixture.root.join("outside/child.ts"),
            "export const child = 1;\n",
        )
        .expect("child");
        symlink(
            fixture.root.join("outside"),
            fixture.root.join("rules/nested"),
        )
        .expect("symlink");
        let error = materialize_product(
            &fixture.root,
            &fixture.manifest,
            &fixture.assets,
            &toolchain(),
            MaterializationLimits::default(),
        )
        .expect_err("symlink must fail");
        assert_eq!(error.diagnostic().code(), "MATERIALIZER_SOURCE_SYMLINK");
        fixture.cleanup();
    }

    #[test]
    fn permissioned_runner_denies_outside_reads_and_kills_output_overflow() {
        let temporary = tempfile::tempdir().expect("temporary");
        let node = node_from_path();
        let denial = temporary.path().join("denial.mjs");
        fs::write(
            &denial,
            "await (await import('node:fs/promises')).readFile('/etc/passwd');\n",
        )
        .expect("denial script");
        let output = run_node(
            &node,
            &denial,
            &[],
            temporary.path(),
            MaterializationLimits::default(),
        )
        .expect("permission process");
        assert!(!output.status.success(), "outside read must be denied");
        let flood = temporary.path().join("flood.mjs");
        fs::write(
            &flood,
            "process.stdout.write('x'.repeat(65537)); setInterval(() => {}, 1000);\n",
        )
        .expect("flood script");
        let error = run_node(
            &node,
            &flood,
            &[],
            temporary.path(),
            MaterializationLimits::default(),
        )
        .expect_err("output flood must fail");
        assert_eq!(error.diagnostic().code(), "MATERIALIZER_TOOL_OUTPUT_BOUNDS");
    }

    fn node_from_path() -> PathBuf {
        let path = std::env::var_os("PATH").expect("PATH");
        let executable = if cfg!(windows) { "node.exe" } else { "node" };
        std::env::split_paths(&path)
            .map(|directory| directory.join(executable))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| panic!("{executable} executable must be available on PATH"))
    }

    fn file_bytes(files: &[PublicationFile]) -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|file| {
                (
                    file.relative_path().as_str().to_owned(),
                    file.bytes().to_vec(),
                )
            })
            .collect()
    }

    struct Fixture {
        root: PathBuf,
        manifest: ProductManifest,
        assets: EngineAssets,
    }
    impl Fixture {
        fn new(label: &str, source: &[(&str, &str)]) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("rusty-product-materializer-{label}-{nonce}"));
            fs::create_dir_all(root.join("content")).expect("content");
            for (path, text) in source {
                let output = root.join(path);
                fs::create_dir_all(output.parent().expect("parent")).expect("lane");
                fs::write(output, text).expect("source");
            }
            if !root.join("rules/extra.ts").exists() {
                fs::create_dir_all(root.join("rules")).expect("rules");
                fs::write(root.join("rules/extra.ts"), "export default { intentDescriptors: [], inputMap: [], gameplayDefinitions: [], timelines: [], capabilityBindings: [] };\n").expect("default fragment");
            }
            if !root.join("runtime/main.ts").exists() {
                fs::create_dir_all(root.join("runtime")).expect("runtime");
                fs::write(root.join("runtime/main.ts"), "export default { initialize: () => ({}), turn: ({ state }: { state: unknown }) => state, project: ({ state }: { state: unknown }) => state };\n").expect("default runtime");
            }
            let manifest = decode_product_manifest(MANIFEST).expect("manifest");
            Self {
                root,
                manifest,
                assets: test_assets(),
            }
        }
        fn cleanup(self) {
            let _ = fs::remove_dir_all(self.root);
        }
    }
    const MANIFEST: &str = r#"[product]
id = "rusty.test"
[runtime_composition]
entrypoints = ["rules/main.ts", "rules/extra.ts"]
[lifecycle]
mode = "demand"
[runtime]
entry = "runtime/main.ts"
[ui]
entry = "ui/main.ts"
[content]
root = "content"
[outputs]
compiled_composition = "generated/compiled-composition.json"
admitted_runtime_content = "generated/runtime-content"
product_assembly = "generated/product-assembly"
product_bundle = "generated/product-bundle"
"#;
    fn test_assets() -> EngineAssets {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let mut assets = Vec::new();
        for (package, directory) in [
            (
                "@rusty-engine/runtime-composition-authoring",
                workspace.join("rules/packages/runtime-composition-authoring"),
            ),
            (
                "@rusty-engine/product-browser-host",
                workspace.join("render/artifacts/product-browser-host"),
            ),
            (
                "@rusty-engine/application-host",
                workspace.join("render/artifacts/application-host"),
            ),
        ] {
            let base = if package.ends_with("authoring") {
                directory.join("dist")
            } else {
                directory.clone()
            };
            if package.ends_with("authoring") {
                assets.push(
                    EngineAsset::new(
                        package,
                        "package.json",
                        fs::read(directory.join("package.json")).expect("authoring package"),
                    )
                    .expect("asset"),
                );
            }
            for entry in fs::read_dir(base).expect("artifact directory") {
                let entry = entry.expect("artifact entry");
                let path = entry.path();
                if path.is_file()
                    && (path.extension().and_then(|v| v.to_str()) == Some("js")
                        || entry.file_name().to_string_lossy().ends_with(".d.ts"))
                {
                    assets.push(
                        EngineAsset::new(
                            package,
                            if package.ends_with("authoring") {
                                format!("dist/{}", entry.file_name().to_string_lossy())
                            } else {
                                entry.file_name().to_string_lossy().to_string()
                            },
                            fs::read(path).expect("artifact file"),
                        )
                        .expect("asset"),
                    );
                }
            }
            if !package.ends_with("authoring") {
                assets.push(
                    EngineAsset::new(
                        package,
                        "package.json",
                        fs::read(directory.join("package.json")).expect("host package"),
                    )
                    .expect("asset"),
                );
            }
        }
        EngineAssets::new(assets).expect("assets")
    }
    fn toolchain() -> MaterializationToolchain {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        MaterializationToolchain::new(
            node_from_path(),
            workspace.join("rules/node_modules/typescript/lib/typescript.js"),
            workspace.join("render/node_modules/vite/bin/vite.js"),
        )
        .with_temporary_parent(workspace.join("render"))
    }
    fn plain_toolchain() -> MaterializationToolchain {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        MaterializationToolchain::new(
            node_from_path(),
            workspace.join("rules/node_modules/typescript/lib/typescript.js"),
            workspace.join("render/node_modules/vite/bin/vite.js"),
        )
    }
}
