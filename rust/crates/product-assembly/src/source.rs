use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path, PathBuf},
};

use content_store::{
    decode_manifest, encode_manifest, ArtifactClass, ArtifactRole, ContentManifest,
};
use product_model::{
    admit_checked_product_composition, decode_compiled_composition,
    link_admitted_product_composition, CapabilityAvailability, CapabilityKind, CapabilityUse,
    ProductKernelCapabilityDescriptor, ProductManifest, ProductPath,
};
use serde::Serialize;

use crate::{
    error::ProductAssemblyError,
    filesystem::{read_product_file, read_product_tree, total_bytes, ProductFile},
    publish::{verify_outputs, AssemblyPublication, PublicationFile, PublicationOutput},
    receipt::{decode_assembly_receipt, AssemblyClosureEntry, AssemblyEntryKind, AssemblyReceipt},
    MAX_GENERATED_FILES, MAX_GENERATED_SOURCE_BYTES,
};

const MANIFEST_PATH: &str = "rusty.toml";
// Legacy Product Assembly locates the shared content-manifest schema through
// this product-root-relative source path. The active host-neutral ContentStore
// owns `content_store::CONTENT_MANIFEST_PATH` instead; neither owner aliases or
// probes the other's storage envelope.
const LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH: &str = "content/manifest.json";
const BROWSER_INDEX_PATH: &str = "index.html";
const BROWSER_MAIN_PATH: &str = "main.js";
const BROWSER_BRIDGE_PATH: &str = "bridge.js";
const BROWSER_ENGINE_PATH: &str = "engine/product-browser-host.js";
const BROWSER_RUNTIME_ADAPTER_PATH: &str = "runtime-adapter.js";
const BROWSER_RENDERER_PRELOAD_PATH: &str = "renderer-preload.json";
const VM_RUNTIME_PROGRAM_PATH: &str = "runtime/main.js";
const RENDERER_TEXTURE_ROLE: &str = "resource:renderer-texture";
const RENDERER_AUDIO_ROLE: &str = "resource:renderer-audio";
const RENDERER_MESH_ROLE: &str = "resource:renderer-mesh";
const RENDERER_TEXTURE_MAX_COUNT: usize = 256;
const RENDERER_TEXTURE_MAX_BYTES: usize = 16 * 1024 * 1024;
const RENDERER_AUDIO_MAX_COUNT: usize = 64;
const RENDERER_AUDIO_MAX_BYTES: usize = 8 * 1024 * 1024;
const RENDERER_MESH_MAX_COUNT: usize = 1024;
const RENDERER_MESH_MAX_BYTES: usize = crate::MAX_ASSEMBLY_FILE_BYTES;
const RENDERER_MESH_MAX_TOTAL_BYTES: usize = crate::MAX_ASSEMBLY_TOTAL_BYTES;

/// The generated, product-specific Rust source plan. The returned source is
/// ordinary closed Rust; no callback table, registry, or erased function is
/// retained by the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblySourcePlan {
    cargo_toml: Vec<u8>,
    lib_rs: Vec<u8>,
    main_rs: Vec<u8>,
    product_rs: Vec<u8>,
    kernel_entry: Option<String>,
    kernel_package: Option<String>,
    runtime_program_path: Option<String>,
    compiled_composition_path: String,
}

/// Fresh, typed browser bundle bytes supplied by the materializer/host.
///
/// The fixed roots are Engine-owned composition files. The declared `ui`
/// entry and any additional files must stay in the bounded `ui/` or `assets/`
/// lanes; no source maps, TypeScript, package-manager/configuration files,
/// node_modules, or absolute module/resource imports are admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserBundleInputs {
    ui_entry: ProductPath,
    files: Vec<PublicationFile>,
}

impl BrowserBundleInputs {
    pub fn new(
        ui_entry: impl Into<String>,
        mut files: Vec<PublicationFile>,
    ) -> Result<Self, ProductAssemblyError> {
        let ui_entry = ProductPath::parse(ui_entry.into()).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_BROWSER_UI_ENTRY",
                "product-bundle",
                error.to_string(),
            )
        })?;
        if !ui_entry.as_str().starts_with("ui/") || !ui_entry.as_str().ends_with(".js") {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_BROWSER_UI_ENTRY",
                ui_entry.as_str(),
                "compiled UI entry must be a product-relative ui/*.js module",
            ));
        }
        files.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
        if files.len() > MAX_GENERATED_FILES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_BROWSER_FILE_COUNT",
                "product-bundle",
                format!("browser bundle is limited to {MAX_GENERATED_FILES} files"),
            ));
        }
        let total_bytes = files.iter().try_fold(0usize, |total, file| {
            total.checked_add(file.bytes().len()).ok_or_else(|| {
                ProductAssemblyError::new(
                    "ASSEMBLY_BROWSER_BYTES",
                    "product-bundle",
                    "browser bundle byte accounting overflowed",
                )
            })
        })?;
        if total_bytes > crate::MAX_ASSEMBLY_TOTAL_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_BROWSER_BYTES",
                "product-bundle",
                format!(
                    "browser bundle is limited to {} bytes",
                    crate::MAX_ASSEMBLY_TOTAL_BYTES
                ),
            ));
        }
        let mut seen = BTreeSet::new();
        for file in &files {
            let path = file.relative_path().as_str();
            if !seen.insert(path) {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_BROWSER_DUPLICATE_FILE",
                    path,
                    "browser bundle files must have unique product-relative paths",
                ));
            }
            validate_browser_path(path)?;
            validate_browser_bytes(path, file.bytes())?;
        }
        for required in [
            BROWSER_INDEX_PATH,
            BROWSER_MAIN_PATH,
            BROWSER_BRIDGE_PATH,
            BROWSER_ENGINE_PATH,
            BROWSER_RUNTIME_ADAPTER_PATH,
            ui_entry.as_str(),
        ] {
            if !seen.contains(required) {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_BROWSER_REQUIRED_FILE",
                    required,
                    "complete browser bundle is missing a required Engine or compiled UI root",
                ));
            }
        }
        require_browser_reference(&files, BROWSER_INDEX_PATH, "./main.js")?;
        require_browser_reference(
            &files,
            BROWSER_MAIN_PATH,
            &format!("./{}", ui_entry.as_str()),
        )?;
        require_browser_reference(
            &files,
            BROWSER_BRIDGE_PATH,
            &format!("./{BROWSER_RUNTIME_ADAPTER_PATH}"),
        )?;
        Ok(Self { ui_entry, files })
    }

    pub fn ui_entry(&self) -> &ProductPath {
        &self.ui_entry
    }

    pub fn files(&self) -> &[PublicationFile] {
        &self.files
    }
}

/// Fresh materialized inputs supplied by the product command before planning.
/// The compiler/host owns how TypeScript is built; this crate only admits the
/// bounded bytes and publishes them into product-relative generated lanes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyGenerationInputs {
    compiled_composition: Vec<u8>,
    browser_bundle: Option<BrowserBundleInputs>,
    engine_dependency_path: Option<String>,
    kernel_dependency_path: Option<String>,
    runtime_program: Option<Vec<u8>>,
}

impl AssemblyGenerationInputs {
    pub fn new(compiled_composition: Vec<u8>) -> Result<Self, ProductAssemblyError> {
        if compiled_composition.len() > product_model::MAX_COMPILED_COMPOSITION_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_COMPOSITION_BYTES",
                "compiled-composition.json",
                "compiled composition exceeds its bounded admission size",
            ));
        }
        Ok(Self {
            compiled_composition,
            browser_bundle: None,
            engine_dependency_path: None,
            kernel_dependency_path: None,
            runtime_program: None,
        })
    }

    pub fn with_browser_bundle(
        mut self,
        browser_bundle: BrowserBundleInputs,
    ) -> Result<Self, ProductAssemblyError> {
        self.browser_bundle = Some(browser_bundle);
        Ok(self)
    }

    /// Supplies a product-relative Cargo path from generated/product-assembly
    /// to the Rusty Engine facade. Fresh generation requires this path so the
    /// generated package is a standalone Cargo package rather than depending
    /// on an ambient product workspace.
    pub fn with_engine_dependency_path(
        mut self,
        path: impl Into<String>,
    ) -> Result<Self, ProductAssemblyError> {
        let path = path.into();
        validate_dependency_path(&path, "ASSEMBLY_ENGINE_DEPENDENCY_PATH")?;
        self.engine_dependency_path = Some(path);
        Ok(self)
    }

    /// Supplies the optional product-relative Cargo path used by a
    /// source-linked Product Kernel module. It is required when the manifest
    /// declares `kernel.entry` so generated code has no ambient workspace
    /// dependency.
    pub fn with_kernel_dependency_path(
        mut self,
        path: impl Into<String>,
    ) -> Result<Self, ProductAssemblyError> {
        let path = path.into();
        validate_dependency_path(&path, "ASSEMBLY_KERNEL_DEPENDENCY_PATH")?;
        self.kernel_dependency_path = Some(path);
        Ok(self)
    }

    /// Supplies the one already-bundled JavaScript program admitted for a
    /// manifest selecting the Engine-owned VM runtime lane.
    pub fn with_runtime_program(mut self, program: Vec<u8>) -> Result<Self, ProductAssemblyError> {
        if program.is_empty() || program.len() > crate::MAX_ASSEMBLY_FILE_BYTES {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_RUNTIME_PROGRAM_BYTES",
                VM_RUNTIME_PROGRAM_PATH,
                "runtime program must be non-empty and fit within one admitted assembly file",
            ));
        }
        std::str::from_utf8(&program).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_RUNTIME_PROGRAM_UTF8",
                VM_RUNTIME_PROGRAM_PATH,
                format!("runtime program must be UTF-8 JavaScript: {error}"),
            )
        })?;
        self.runtime_program = Some(program);
        Ok(self)
    }

    pub fn compiled_composition(&self) -> &[u8] {
        &self.compiled_composition
    }

    pub fn browser_bundle(&self) -> Option<&BrowserBundleInputs> {
        self.browser_bundle.as_ref()
    }

    pub fn engine_dependency_path(&self) -> Option<&str> {
        self.engine_dependency_path.as_deref()
    }

    pub fn kernel_dependency_path(&self) -> Option<&str> {
        self.kernel_dependency_path.as_deref()
    }

    pub fn runtime_program(&self) -> Option<&[u8]> {
        self.runtime_program.as_deref()
    }
}

impl AssemblySourcePlan {
    pub fn cargo_toml(&self) -> &[u8] {
        &self.cargo_toml
    }

    pub fn lib_rs(&self) -> &[u8] {
        &self.lib_rs
    }

    pub fn main_rs(&self) -> &[u8] {
        &self.main_rs
    }

    pub fn product_rs(&self) -> &[u8] {
        &self.product_rs
    }

    pub fn kernel_entry(&self) -> Option<&str> {
        self.kernel_entry.as_deref()
    }

    pub fn kernel_package(&self) -> Option<&str> {
        self.kernel_package.as_deref()
    }

    /// Fixed generated location of the Engine-owned VM bundle, when selected.
    pub fn runtime_program_path(&self) -> Option<&str> {
        self.runtime_program_path.as_deref()
    }

    pub fn compiled_composition_path(&self) -> &str {
        &self.compiled_composition_path
    }
}

/// All output bytes and the deterministic receipt prepared before any output
/// is touched. Calling [`AssemblyPlan::publish`] is the only operation that
/// changes the product root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyPlan {
    receipt: AssemblyReceipt,
    source_plan: AssemblySourcePlan,
    publication: AssemblyPublication,
}

/// The two deliberately non-interchangeable Product Kernel authoring modes.
/// A package is a Cargo dependency in the generated closure; it is never
/// reduced to one `#[path]` source module.
#[derive(Debug, Clone, PartialEq, Eq)]
enum KernelSource {
    Entry(ProductPath),
    Package(KernelPackage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KernelPackage {
    manifest_path: ProductPath,
    package_name: String,
    cargo_rewrites: BTreeMap<ProductPath, Vec<u8>>,
}

impl KernelSource {
    fn from_manifest(
        manifest: &ProductManifest,
        files: &BTreeMap<String, ProductFile>,
        engine_dependency_path: &str,
    ) -> Result<Option<Self>, ProductAssemblyError> {
        match (manifest.kernel_entry(), manifest.kernel_package()) {
            (Some(entry), None) => Ok(Some(Self::Entry(entry.clone()))),
            (None, Some(package)) => Ok(Some(Self::Package(admit_kernel_package(
                package,
                files,
                engine_dependency_path,
            )?))),
            (None, None) => Ok(None),
            // Product Model rejects this before Assembly, but retain the
            // boundary here so constructed values cannot select ambiguity.
            (Some(_), Some(_)) => Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_MODE_CONFLICT",
                "kernel",
                "declare exactly one of kernel.entry or kernel.package",
            )),
        }
    }
}

/// Admits the complete Cargo package closure without using Cargo metadata or
/// an ambient workspace. Every local path dependency must resolve beneath the
/// `kernel/` lane and every admitted package receives the same explicit Engine
/// facade path when copied into the generated Assembly.
fn admit_kernel_package(
    root_manifest: &ProductPath,
    files: &BTreeMap<String, ProductFile>,
    engine_dependency_path: &str,
) -> Result<KernelPackage, ProductAssemblyError> {
    if !root_manifest.as_str().starts_with("kernel/") {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_PACKAGE_LANE",
            root_manifest.as_str(),
            "kernel.package must stay inside the fixed kernel/ lane",
        ));
    }
    let mut pending = vec![root_manifest.clone()];
    let mut visited = BTreeSet::new();
    let mut rewrites = BTreeMap::new();
    let mut root_name = None;
    while let Some(manifest_path) = pending.pop() {
        if !visited.insert(manifest_path.clone()) {
            continue;
        }
        let file = files.get(manifest_path.as_str()).ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_MISSING",
                manifest_path.as_str(),
                "declared Product Kernel Cargo manifest is absent from the admitted kernel lane",
            )
        })?;
        let text = std::str::from_utf8(&file.bytes).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_UTF8",
                manifest_path.as_str(),
                format!("Product Kernel Cargo manifest must be UTF-8: {error}"),
            )
        })?;
        let mut document: toml::Value = toml::from_str(text).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_TOML",
                manifest_path.as_str(),
                format!("invalid Product Kernel Cargo manifest: {error}"),
            )
        })?;
        if let Some(workspace) = document.get("workspace") {
            let empty_root_workspace = manifest_path == *root_manifest
                && workspace.as_table().is_some_and(toml::map::Map::is_empty);
            if !empty_root_workspace {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_WORKSPACE",
                    manifest_path.as_str(),
                    "only the root Product Kernel package may use an exact empty [workspace] to opt out of an outer workspace",
                ));
            }
            // An exact empty root workspace is an authored opt-out from the
            // product's enclosing workspace. The generated Assembly is
            // already detached, so retaining it here would create a forbidden
            // nested Cargo workspace root.
            document
                .as_table_mut()
                .expect("Cargo manifest is a TOML table")
                .remove("workspace");
        }
        if contains_workspace_inheritance(&document) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_WORKSPACE",
                manifest_path.as_str(),
                "Product Kernel packages may not inherit an ambient Cargo workspace",
            ));
        }
        if document
            .get("package")
            .and_then(toml::Value::as_table)
            .is_some_and(|package| package.get("build").is_some())
            || document.get("build-dependencies").is_some()
            || document.get("dev-dependencies").is_some()
            || document.get("target").is_some()
        {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_UNBOUNDED_BUILD",
                manifest_path.as_str(),
                "Product Kernel packages may not use build, dev, or target-specific dependency closures",
            ));
        }
        let package = document
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_IDENTITY",
                    manifest_path.as_str(),
                    "Product Kernel Cargo manifest requires a [package] table",
                )
            })?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_IDENTITY",
                    manifest_path.as_str(),
                    "Product Kernel Cargo package requires a string package.name",
                )
            })?;
        validate_cargo_package_name(name, manifest_path.as_str())?;
        if manifest_path == *root_manifest {
            root_name = Some(name.to_owned());
        }
        let package_dir = manifest_path_parent(&manifest_path)?;
        let Some(dependencies) = document.get_mut("dependencies") else {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_ENGINE_DEPENDENCY",
                manifest_path.as_str(),
                "Product Kernel package must declare the fixed rusty-engine facade dependency",
            ));
        };
        let dependencies = dependencies.as_table_mut().ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_DEPENDENCIES",
                manifest_path.as_str(),
                "Product Kernel dependencies must be a TOML table",
            )
        })?;
        let mut has_engine = false;
        for (dependency_name, dependency) in dependencies.iter_mut() {
            if dependency_name == "rusty-engine" {
                if !dependency.is_table() {
                    return Err(ProductAssemblyError::new(
                        "ASSEMBLY_KERNEL_PACKAGE_ENGINE_DEPENDENCY",
                        manifest_path.as_str(),
                        "rusty-engine must be an explicit table dependency so Assembly can bind the exact facade",
                    ));
                }
                *dependency = toml::Value::Table(toml::map::Map::from_iter([(
                    "path".to_owned(),
                    toml::Value::String(copied_engine_path(&package_dir, engine_dependency_path)),
                )]));
                has_engine = true;
                continue;
            }
            let table = dependency.as_table().ok_or_else(|| ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_REGISTRY_DEPENDENCY",
                manifest_path.as_str(),
                format!("dependency `{dependency_name}` must be a local path dependency; registry versions are not admitted"),
            ))?;
            let path =
                table
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .ok_or_else(|| {
                        ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_REGISTRY_DEPENDENCY",
                    manifest_path.as_str(),
                    format!("dependency `{dependency_name}` must name a local path inside kernel/"),
                )
                    })?;
            let dependency_manifest =
                local_dependency_manifest(&package_dir, path, manifest_path.as_str())?;
            if !files.contains_key(dependency_manifest.as_str()) {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_LOCAL_DEPENDENCY",
                    manifest_path.as_str(),
                    format!("dependency `{dependency_name}` does not resolve to an admitted kernel Cargo.toml"),
                ));
            }
            pending.push(dependency_manifest);
        }
        if !has_engine {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_ENGINE_DEPENDENCY",
                manifest_path.as_str(),
                "Product Kernel package must declare the fixed rusty-engine facade dependency",
            ));
        }
        let rewritten = toml::to_string(&document).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_TOML",
                manifest_path.as_str(),
                format!("cannot canonicalize admitted Product Kernel Cargo manifest: {error}"),
            )
        })?;
        rewrites.insert(manifest_path, rewritten.into_bytes());
    }
    Ok(KernelPackage {
        manifest_path: root_manifest.clone(),
        package_name: root_name.expect("root kernel manifest always visited"),
        cargo_rewrites: rewrites,
    })
}

fn contains_workspace_inheritance(value: &toml::Value) -> bool {
    match value {
        toml::Value::Table(table) => table
            .iter()
            .any(|(key, value)| key == "workspace" || contains_workspace_inheritance(value)),
        toml::Value::Array(values) => values.iter().any(contains_workspace_inheritance),
        _ => false,
    }
}

fn validate_cargo_package_name(name: &str, path: &str) -> Result<(), ProductAssemblyError> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_PACKAGE_IDENTITY",
            path,
            "Product Kernel package.name must be a bounded ASCII Cargo package identity",
        ));
    }
    Ok(())
}

fn manifest_path_parent(path: &ProductPath) -> Result<ProductPath, ProductAssemblyError> {
    let parent = path
        .as_str()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_PATH",
                path.as_str(),
                "Cargo manifest has no package directory",
            )
        })?;
    ProductPath::parse(parent.to_owned()).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_PACKAGE_PATH",
            path.as_str(),
            error.to_string(),
        )
    })
}

fn local_dependency_manifest(
    package_dir: &ProductPath,
    path: &str,
    logical_path: &str,
) -> Result<ProductPath, ProductAssemblyError> {
    let joined = Path::new(package_dir.as_str())
        .join(path)
        .join("Cargo.toml");
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ProductAssemblyError::new(
                        "ASSEMBLY_KERNEL_PACKAGE_ESCAPE",
                        logical_path,
                        "a Product Kernel local dependency escapes the kernel/ lane",
                    ));
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_KERNEL_PACKAGE_ESCAPE",
                    logical_path,
                    "a Product Kernel local dependency must be relative and remain inside kernel/",
                ))
            }
        }
    }
    let value = normalized
        .to_str()
        .ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_PATH",
                logical_path,
                "Product Kernel dependency path must be UTF-8",
            )
        })?
        .replace('\\', "/");
    let manifest = ProductPath::parse(value).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_PACKAGE_PATH",
            logical_path,
            error.to_string(),
        )
    })?;
    if !manifest.as_str().starts_with("kernel/") {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_PACKAGE_ESCAPE",
            logical_path,
            "a Product Kernel local dependency escapes the kernel/ lane",
        ));
    }
    Ok(manifest)
}

fn copied_engine_path(package_dir: &ProductPath, root_engine_path: &str) -> String {
    let depth = package_dir.as_str().split('/').count();
    format!("{}{}", "../".repeat(depth), root_engine_path)
}

/// Package mode copies an authored source closure, never Cargo build output,
/// lock snapshots, VCS state, or per-machine caches. Reject rather than skip
/// those paths so receipt identity cannot hide product-local build residue.
fn validate_kernel_package_source_closure(
    files: &BTreeMap<String, ProductFile>,
) -> Result<(), ProductAssemblyError> {
    for file in files.values() {
        let path = file.relative_path.as_str();
        let Some(kernel_relative) = path.strip_prefix("kernel/") else {
            continue;
        };
        let rejected_component = kernel_relative.split('/').find(|component| {
            matches!(
                *component,
                "target" | ".git" | ".cargo" | ".cache" | "cache"
            )
        });
        if rejected_component.is_some() || kernel_relative == "Cargo.lock" {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_PACKAGE_NON_SOURCE",
                path,
                "Product Kernel package closure may contain authored source only; remove Cargo.lock, target, VCS, or cache output and direct Cargo target output outside kernel/",
            ));
        }
    }
    Ok(())
}

impl AssemblyPlan {
    pub fn receipt(&self) -> &AssemblyReceipt {
        &self.receipt
    }

    pub fn source_plan(&self) -> &AssemblySourcePlan {
        &self.source_plan
    }

    pub fn publication(&self) -> &AssemblyPublication {
        &self.publication
    }

    pub fn receipt_bytes(&self) -> Result<Vec<u8>, ProductAssemblyError> {
        self.receipt.json_bytes()
    }

    pub fn publish(
        &self,
        product_root: &Path,
    ) -> Result<crate::PublishedOutputs, ProductAssemblyError> {
        crate::publish_outputs(product_root, &self.publication)
    }
}

/// Reads authored lanes, strictly admits fresh compiler/host bytes, and
/// prepares one deterministic Product Assembly. No generated output is used
/// as a generation input.
pub fn plan_product_assembly(
    product_root: &Path,
    manifest: &ProductManifest,
    inputs: &AssemblyGenerationInputs,
) -> Result<AssemblyPlan, ProductAssemblyError> {
    plan_product_assembly_with_kernel_capabilities(product_root, manifest, inputs, &[])
}

/// Variant used by a downstream source-linked Product Kernel declaration. The
/// descriptor slice is static metadata only; it is copied into generated Rust
/// as a closed const slice and never becomes a runtime registry.
pub fn plan_product_assembly_with_kernel_capabilities(
    product_root: &Path,
    manifest: &ProductManifest,
    inputs: &AssemblyGenerationInputs,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<AssemblyPlan, ProductAssemblyError> {
    let browser_bundle = inputs.browser_bundle().ok_or_else(|| {
        ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_BUNDLE_REQUIRED",
            "product-bundle",
            "fresh Product Assembly generation requires the typed browser host/template closure",
        )
    })?;
    let engine_dependency_path = inputs.engine_dependency_path().ok_or_else(|| {
        ProductAssemblyError::new(
            "ASSEMBLY_ENGINE_DEPENDENCY_REQUIRED",
            "Cargo.toml",
            "fresh Product Assembly generation requires a product-relative Engine dependency path",
        )
    })?;
    let vm_runtime_entry = manifest.runtime_entry();
    let vm_runtime_program = inputs.runtime_program();
    if vm_runtime_entry.is_some() && vm_runtime_program.is_none() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RUNTIME_PROGRAM_REQUIRED",
            VM_RUNTIME_PROGRAM_PATH,
            "a manifest runtime.entry requires one bundled VM runtime program",
        ));
    }
    if vm_runtime_entry.is_none() && vm_runtime_program.is_some() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RUNTIME_ENTRY_REQUIRED",
            VM_RUNTIME_PROGRAM_PATH,
            "a bundled VM runtime program requires manifest runtime.entry",
        ));
    }
    if vm_runtime_entry.is_some() && manifest.has_kernel() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RUNTIME_MODE_CONFLICT",
            "rusty.toml",
            "runtime.entry cannot be combined with a Product Kernel runtime mode",
        ));
    }
    if manifest.has_kernel() && inputs.kernel_dependency_path().is_none() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_DEPENDENCY_REQUIRED",
            "Cargo.toml",
            "a source-linked Product Kernel requires a product-relative kernel dependency path",
        ));
    }
    if !manifest.has_kernel() && !kernel_capabilities.is_empty() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_KERNEL_ENTRY_REQUIRED",
            "rusty.toml",
            "Product Kernel descriptors require a declared kernel.entry or kernel.package runtime definition",
        ));
    }
    let root = crate::filesystem::checked_product_root(product_root)?;
    let mut source_files = BTreeMap::new();
    let manifest_path = ProductPath::parse(MANIFEST_PATH.to_owned()).expect("fixed path");
    let manifest_file = read_product_file(&root, &manifest_path, MANIFEST_PATH)?;
    let manifest_text = std::str::from_utf8(&manifest_file.bytes).map_err(|error| {
        ProductAssemblyError::new("ASSEMBLY_MANIFEST_UTF8", MANIFEST_PATH, error.to_string())
    })?;
    let disk_manifest = product_model::decode_product_manifest(manifest_text).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_MANIFEST_ADMISSION",
            MANIFEST_PATH,
            error.to_string(),
        )
    })?;
    if disk_manifest != *manifest {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_MANIFEST_STALE",
            MANIFEST_PATH,
            "the supplied Product Layout differs from rusty.toml on disk",
        ));
    }
    insert_file(&mut source_files, manifest_file)?;

    let source_paths = manifest.composition_entrypoints().to_vec();
    // Admit complete bounded authoring lanes instead of copying only declared
    // entrypoints. This conservatively closes ordinary relative imports while
    // keeping cache/build output out of the runtime-content contract.
    let mut source_lanes = BTreeSet::from([
        ProductPath::parse("rules".to_owned()).expect("fixed source lane"),
        ProductPath::parse("ui".to_owned()).expect("fixed source lane"),
    ]);
    if manifest.has_kernel() {
        source_lanes.insert(ProductPath::parse("kernel".to_owned()).expect("fixed source lane"));
    }
    if vm_runtime_entry.is_some() {
        source_lanes.insert(ProductPath::parse("runtime".to_owned()).expect("fixed runtime lane"));
    }
    for lane in source_lanes {
        let logical = lane.to_string();
        for file in read_product_tree(&root, &lane, &logical)? {
            insert_file(&mut source_files, file)?;
        }
    }
    let mut required_sources = source_paths;
    if let Some(kernel) = manifest.kernel_entry() {
        required_sources.push(kernel.clone());
    }
    if let Some(kernel) = manifest.kernel_package() {
        required_sources.push(kernel.clone());
    }
    if let Some(runtime) = vm_runtime_entry {
        required_sources.push(runtime.clone());
    }
    required_sources.push(manifest.ui_entry().clone());
    for path in required_sources {
        if !source_files.contains_key(path.as_str()) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_SOURCE_ENTRYPOINT_MISSING",
                path.as_str(),
                "declared Product Layout entrypoint is not present in the admitted source closure",
            ));
        }
    }

    let content = collect_content(&root, manifest)?;
    if let Some(file) = content.manifest_source {
        insert_file(&mut source_files, file)?;
    }
    if manifest.kernel_package().is_some() {
        validate_kernel_package_source_closure(&source_files)?;
    }
    let kernel_source =
        KernelSource::from_manifest(manifest, &source_files, engine_dependency_path)?;
    let source_values = source_files.into_values().collect::<Vec<_>>();
    total_bytes(source_values.iter())?;

    let compiled_path = manifest.compiled_composition_output().clone();
    let compiled_logical = compiled_path.to_string();
    let decoded = decode_compiled_composition(inputs.compiled_composition()).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_COMPOSITION_DECODE",
            &compiled_logical,
            error.to_string(),
        )
    })?;
    let admitted =
        admit_checked_product_composition(manifest, decoded.clone()).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_COMPOSITION_ADMISSION",
                &compiled_logical,
                error.to_string(),
            )
        })?;
    let canonical_composition = admitted.canonical_bytes().to_vec();
    let linked =
        link_admitted_product_composition(admitted, kernel_capabilities).map_err(|error| {
            ProductAssemblyError::new(
                "ASSEMBLY_COMPOSITION_LINK",
                &compiled_logical,
                error.to_string(),
            )
        })?;
    if vm_runtime_entry.is_some() && !linked.capability_bindings().is_empty() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_VM_CAPABILITY_BINDINGS",
            &compiled_logical,
            "the initial VM runtime lane does not admit Engine or Product Kernel capability bindings",
        ));
    }
    if !linked.capability_bindings().is_empty() && kernel_source.is_none() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RUNTIME_LINKAGE_REQUIRES_KERNEL",
            &compiled_logical,
            "Engine or Product Kernel capability bindings require the fixed RustyProductRuntime definition; the no-kernel empty runtime cannot silently dispatch them",
        ));
    }
    let renderer_preload = PublicationFile::new(
        BROWSER_RENDERER_PRELOAD_PATH,
        render_renderer_preload(&content.renderer_preload)?,
    )?;
    let mut generated_browser_files = browser_bundle.files().to_vec();
    generated_browser_files.push(renderer_preload.clone());
    generated_browser_files.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let source_plan = generate_source_plan(
        manifest,
        kernel_source.as_ref(),
        &canonical_composition,
        kernel_capabilities,
        engine_dependency_path,
        inputs.kernel_dependency_path(),
        &generated_browser_files,
        &content.runtime_files,
        &content.runtime_json_content,
        &content.renderer_preload,
        vm_runtime_program,
    )?;

    let runtime_files = content.runtime_files;
    let runtime_publication_files = runtime_files
        .iter()
        .map(|file| {
            PublicationFile::new(
                file.relative_path
                    .as_str()
                    .strip_prefix("content/")
                    .unwrap_or(file.relative_path.as_str()),
                file.bytes.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut assembly_files = vec![
        PublicationFile::new("Cargo.toml", source_plan.cargo_toml.clone())?,
        PublicationFile::new("src/lib.rs", source_plan.lib_rs.clone())?,
        PublicationFile::new("src/main.rs", source_plan.main_rs.clone())?,
        PublicationFile::new("src/product.rs", source_plan.product_rs.clone())?,
        PublicationFile::new(
            "artifacts/compiled-composition.json",
            canonical_composition.clone(),
        )?,
    ];
    if let Some(program) = vm_runtime_program {
        assembly_files.push(PublicationFile::new(
            VM_RUNTIME_PROGRAM_PATH,
            program.to_vec(),
        )?);
    }
    for file in &source_values {
        assembly_files.push(PublicationFile::new(
            format!("source/{}", file.relative_path),
            file.bytes.clone(),
        )?);
    }
    if let Some(KernelSource::Package(package)) = &kernel_source {
        for file in &source_values {
            let Some(relative) = file.relative_path.as_str().strip_prefix("kernel/") else {
                continue;
            };
            let bytes = package
                .cargo_rewrites
                .get(&file.relative_path)
                .cloned()
                .unwrap_or_else(|| file.bytes.clone());
            assembly_files.push(PublicationFile::new(format!("kernel/{relative}"), bytes)?);
        }
    }
    for file in &runtime_files {
        assembly_files.push(PublicationFile::new(
            format!(
                "content/{}",
                file.relative_path
                    .as_str()
                    .strip_prefix("content/")
                    .unwrap_or(file.relative_path.as_str())
            ),
            file.bytes.clone(),
        )?);
    }
    validate_generated_count(&assembly_files)?;

    // The browser bundle is deliberately a runtime-only copy. Host templates
    // may add files later; those files enter the same receipt role rather than
    // becoming hidden reach-throughs into authoring sources.
    let mut bundle_files = vec![PublicationFile::new(
        "artifacts/compiled-composition.json",
        canonical_composition.clone(),
    )?];
    for file in &runtime_files {
        bundle_files.push(PublicationFile::new(
            format!(
                "content/{}",
                file.relative_path
                    .as_str()
                    .strip_prefix("content/")
                    .unwrap_or(file.relative_path.as_str())
            ),
            file.bytes.clone(),
        )?);
    }
    for file in &generated_browser_files {
        bundle_files.push(file.clone());
    }
    validate_generated_count(&bundle_files)?;

    let mut receipt_entries = Vec::new();
    for file in &source_values {
        receipt_entries.push(AssemblyClosureEntry::new(
            AssemblyEntryKind::AuthoredSource,
            file.relative_path.as_str(),
            &file.bytes,
        ));
    }
    receipt_entries.push(AssemblyClosureEntry::new(
        AssemblyEntryKind::CompiledComposition,
        compiled_path.as_str(),
        &canonical_composition,
    ));
    for file in &runtime_publication_files {
        receipt_entries.push(AssemblyClosureEntry::new(
            AssemblyEntryKind::RuntimeContent,
            format!(
                "{}/{}",
                manifest.admitted_runtime_content_output(),
                file.relative_path()
            ),
            file.bytes(),
        ));
    }
    for file in &assembly_files {
        receipt_entries.push(AssemblyClosureEntry::new(
            AssemblyEntryKind::ExecutableWorkspace,
            format!(
                "{}/{}",
                manifest.product_assembly_output(),
                file.relative_path()
            ),
            file.bytes(),
        ));
    }
    for file in &bundle_files {
        receipt_entries.push(AssemblyClosureEntry::new(
            AssemblyEntryKind::BrowserBundle,
            format!(
                "{}/{}",
                manifest.product_bundle_output(),
                file.relative_path()
            ),
            file.bytes(),
        ));
    }
    validate_total_receipt_bytes(&receipt_entries)?;
    let receipt = AssemblyReceipt::new(manifest.product_id(), receipt_entries)?;
    let receipt_bytes = receipt.json_bytes()?;

    assembly_files.push(PublicationFile::new(
        "assembly.json",
        receipt_bytes.clone(),
    )?);
    bundle_files.push(PublicationFile::new("assembly.json", receipt_bytes)?);
    let publication = AssemblyPublication::new(vec![
        PublicationOutput::file(
            manifest.compiled_composition_output().as_str(),
            canonical_composition,
        )?,
        PublicationOutput::directory(
            manifest.admitted_runtime_content_output().as_str(),
            runtime_publication_files,
        )?,
        PublicationOutput::directory(manifest.product_assembly_output().as_str(), assembly_files)?,
        PublicationOutput::directory(manifest.product_bundle_output().as_str(), bundle_files)?,
    ])?;
    Ok(AssemblyPlan {
        receipt,
        source_plan,
        publication,
    })
}

/// Strictly verifies the published assembly receipt and every generated
/// output against a fresh deterministic plan. This catches stale source,
/// relocation drift, tampered output bytes, extra directory files, and
/// malformed receipt fields without trusting the receipt as its own proof.
pub fn verify_product_assembly(
    product_root: &Path,
    manifest: &ProductManifest,
    inputs: &AssemblyGenerationInputs,
) -> Result<AssemblyReceipt, ProductAssemblyError> {
    verify_product_assembly_with_kernel_capabilities(product_root, manifest, inputs, &[])
}

pub fn verify_product_assembly_with_kernel_capabilities(
    product_root: &Path,
    manifest: &ProductManifest,
    inputs: &AssemblyGenerationInputs,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<AssemblyReceipt, ProductAssemblyError> {
    let plan = plan_product_assembly_with_kernel_capabilities(
        product_root,
        manifest,
        inputs,
        kernel_capabilities,
    )?;
    let root = crate::filesystem::checked_product_root(product_root)?;
    let receipt_path = ProductPath::parse(format!(
        "{}/assembly.json",
        manifest.product_assembly_output().as_str()
    ))
    .map_err(|error| {
        ProductAssemblyError::new("ASSEMBLY_RECEIPT_PATH", "assembly.json", error.to_string())
    })?;
    let receipt_file = read_product_file(&root, &receipt_path, "assembly.json")?;
    let decoded = decode_assembly_receipt(&receipt_file.bytes)?;
    if decoded.product() != manifest.product_id() || decoded != *plan.receipt() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RECEIPT_STALE",
            "assembly.json",
            "published receipt does not match the current admitted source/content/composition closure",
        ));
    }
    let expected_bytes = plan.receipt_bytes()?;
    if receipt_file.bytes != expected_bytes {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RECEIPT_STALE",
            "assembly.json",
            "published receipt bytes are not the deterministic current encoding",
        ));
    }
    verify_outputs(product_root, plan.publication())?;
    Ok(decoded)
}

/// Existing-output convenience verifier. Fresh generation must use
/// [`AssemblyGenerationInputs`] from the compiler/host; this helper merely
/// rehydrates those bytes from an already-published tree for diagnostics and
/// migration checks.
pub fn verify_existing_product_assembly(
    product_root: &Path,
    manifest: &ProductManifest,
) -> Result<AssemblyReceipt, ProductAssemblyError> {
    verify_existing_product_assembly_with_kernel_capabilities(product_root, manifest, &[])
}

pub fn verify_existing_product_assembly_with_kernel_capabilities(
    product_root: &Path,
    manifest: &ProductManifest,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<AssemblyReceipt, ProductAssemblyError> {
    let root = crate::filesystem::checked_product_root(product_root)?;
    let compiled_path = manifest.compiled_composition_output().clone();
    let compiled = read_product_file(&root, &compiled_path, compiled_path.as_str())?;
    let cargo_path = ProductPath::parse(format!(
        "{}/Cargo.toml",
        manifest.product_assembly_output().as_str()
    ))
    .expect("generated Cargo path");
    let cargo = read_product_file(&root, &cargo_path, cargo_path.as_str())?;
    let engine_path =
        extract_cargo_dependency_path(&cargo.bytes, "rusty-engine")?.ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_ENGINE_DEPENDENCY_REQUIRED",
                "Cargo.toml",
                "existing generated Cargo.toml does not contain a direct Engine path dependency",
            )
        })?;
    let kernel_path = extract_cargo_dependency_path(&cargo.bytes, "product-kernel")?;
    let bundle_root = manifest.product_bundle_output().clone();
    let prefix = format!("{}/", bundle_root.as_str());
    let mut bundle_files = Vec::new();
    for file in read_product_tree(&root, &bundle_root, bundle_root.as_str())? {
        let Some(relative) = file.relative_path.as_str().strip_prefix(&prefix) else {
            continue;
        };
        if relative == "assembly.json"
            || relative == BROWSER_RENDERER_PRELOAD_PATH
            || relative.starts_with("artifacts/")
            || relative.starts_with("content/")
        {
            continue;
        }
        bundle_files.push(PublicationFile::new(relative, file.bytes)?);
    }
    let ui_entry = bundle_files
        .iter()
        .find(|file| {
            file.relative_path().as_str().starts_with("ui/")
                && file.relative_path().as_str().ends_with(".js")
        })
        .map(|file| file.relative_path().as_str().to_owned())
        .ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_BROWSER_UI_ENTRY",
                "product-bundle",
                "existing browser bundle does not contain a compiled ui/*.js entry",
            )
        })?;
    let browser_bundle = BrowserBundleInputs::new(ui_entry, bundle_files)?;
    let mut inputs = AssemblyGenerationInputs::new(compiled.bytes)?
        .with_browser_bundle(browser_bundle)?
        .with_engine_dependency_path(engine_path)?;
    if let Some(kernel_path) = kernel_path {
        inputs = inputs.with_kernel_dependency_path(kernel_path)?;
    }
    verify_product_assembly_with_kernel_capabilities(
        product_root,
        manifest,
        &inputs,
        kernel_capabilities,
    )
}

struct ContentClosure {
    manifest_source: Option<ProductFile>,
    runtime_files: Vec<ProductFile>,
    runtime_json_content: Vec<ProductFile>,
    renderer_preload: Vec<RendererPreloadResource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RendererPreloadResource {
    identity: String,
    #[serde(rename = "contentHash")]
    content_hash: String,
    #[serde(rename = "mediaType")]
    media_type: &'static str,
    path: String,
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Serialize)]
struct RendererPreloadDescriptor<'a> {
    artifact: &'static str,
    resources: &'a [RendererPreloadResource],
}

fn collect_content(
    root: &crate::filesystem::ProductRoot,
    manifest: &ProductManifest,
) -> Result<ContentClosure, ProductAssemblyError> {
    let tree = read_product_tree(root, manifest.content_root(), "content")?;
    let manifest_path =
        ProductPath::parse(LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH.to_owned()).expect("fixed path");
    let source_manifest = tree
        .iter()
        .find(|file| file.relative_path == manifest_path)
        .cloned();
    let body_files = tree
        .iter()
        .filter(|file| file.relative_path != manifest_path)
        .filter(|file| file.relative_path.as_str() != "content/.keep")
        .cloned()
        .collect::<Vec<_>>();

    let Some(source_manifest) = source_manifest else {
        if body_files.is_empty() {
            let empty = encode_manifest(&ContentManifest::new(vec![])).map_err(|error| {
                ProductAssemblyError::new(
                    "ASSEMBLY_CONTENT_MANIFEST",
                    LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH,
                    error.to_string(),
                )
            })?;
            return Ok(ContentClosure {
                manifest_source: None,
                runtime_files: vec![ProductFile {
                    relative_path: ProductPath::parse(
                        LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH.to_owned(),
                    )
                    .expect("fixed path"),
                    bytes: empty.into_bytes(),
                }],
                runtime_json_content: Vec::new(),
                renderer_preload: Vec::new(),
            });
        }
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_CONTENT_MANIFEST_REQUIRED",
            LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH,
            format!(
                "non-empty content requires {LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH} with exact body identities"
            ),
        ));
    };
    let text = std::str::from_utf8(&source_manifest.bytes).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_CONTENT_MANIFEST_UTF8",
            LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH,
            error.to_string(),
        )
    })?;
    let decoded = decode_manifest(text).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_CONTENT_MANIFEST_ADMISSION",
            LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH,
            error.to_string(),
        )
    })?;
    let canonical = decoded.canonical();
    let encoded = encode_manifest(&canonical).map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_CONTENT_MANIFEST",
            LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH,
            error.to_string(),
        )
    })?;
    let actual = body_files
        .iter()
        .map(|file| {
            file.relative_path
                .as_str()
                .strip_prefix("content/")
                .unwrap_or(file.relative_path.as_str())
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    let declared = canonical
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .collect::<BTreeSet<_>>();
    let cache_paths = canonical
        .artifacts
        .iter()
        .filter(|artifact| artifact.class == ArtifactClass::Cache)
        .map(|artifact| artifact.path.as_str())
        .collect::<BTreeSet<_>>();
    for path in &actual {
        if cache_paths.contains(path.as_str()) {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CONTENT_CACHE_BODY",
                format!("content/{path}"),
                "cache bodies are explicitly forbidden from Product Assembly publication",
            ));
        }
    }
    if let Some(path) = actual.difference(&declared).next() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_CONTENT_EXTRA_BODY",
            format!("content/{path}"),
            format!("content body is not declared by {LEGACY_ASSEMBLY_CONTENT_MANIFEST_PATH}"),
        ));
    }
    let by_path = body_files
        .into_iter()
        .map(|file| {
            (
                file.relative_path
                    .as_str()
                    .strip_prefix("content/")
                    .unwrap_or(file.relative_path.as_str())
                    .to_owned(),
                file,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut runtime_files = vec![ProductFile {
        relative_path: manifest_path,
        bytes: encoded.into_bytes(),
    }];
    let mut runtime_json_content = Vec::new();
    let mut renderer_preload = Vec::new();
    let mut texture_count = 0_usize;
    let mut audio_count = 0_usize;
    let mut mesh_count = 0_usize;
    let mut mesh_bytes = 0_usize;
    let mut renderer_identities = BTreeMap::new();
    for artifact in canonical.load_required() {
        let Some(file) = by_path.get(&artifact.path) else {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CONTENT_MISSING_BODY",
                format!("content/{}", artifact.path),
                "manifest-required content body is missing",
            ));
        };
        if artifact.byte_len != Some(file.bytes.len() as u64)
            || artifact.content_hash != Some(content_store::ContentHash::of(&file.bytes))
        {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CONTENT_HASH_MISMATCH",
                format!("content/{}", artifact.path),
                "content body does not match its admitted manifest identity",
            ));
        }
        if artifact.path.ends_with(".json") {
            serde_json::from_slice::<serde_json::Value>(&file.bytes).map_err(|error| {
                ProductAssemblyError::new(
                    "ASSEMBLY_RUNTIME_JSON_DECODE",
                    format!("content/{}", artifact.path),
                    error.to_string(),
                )
            })?;
            runtime_json_content.push(file.clone());
        }
        let renderer_kind = match &artifact.role {
            ArtifactRole::Resource(role) if role == RENDERER_TEXTURE_ROLE => Some("texture"),
            ArtifactRole::Resource(role) if role == RENDERER_AUDIO_ROLE => Some("audio"),
            ArtifactRole::Resource(role) if role == RENDERER_MESH_ROLE => Some("mesh"),
            _ => None,
        };
        if let Some(kind) = renderer_kind {
            validate_renderer_preload_resource(kind, &artifact.path, &file.bytes)?;
            match kind {
                "texture" => {
                    texture_count += 1;
                    if texture_count > RENDERER_TEXTURE_MAX_COUNT {
                        return Err(ProductAssemblyError::new(
                            "ASSEMBLY_RENDERER_RESOURCE_COUNT",
                            format!("content/{}", artifact.path),
                            "renderer texture resource count exceeds the application-host bound",
                        ));
                    }
                }
                "audio" => {
                    audio_count += 1;
                    if audio_count > RENDERER_AUDIO_MAX_COUNT {
                        return Err(ProductAssemblyError::new(
                            "ASSEMBLY_RENDERER_RESOURCE_COUNT",
                            format!("content/{}", artifact.path),
                            "renderer audio resource count exceeds the application-host bound",
                        ));
                    }
                }
                "mesh" => {
                    mesh_count += 1;
                    mesh_bytes = mesh_bytes.checked_add(file.bytes.len()).ok_or_else(|| {
                        ProductAssemblyError::new(
                            "ASSEMBLY_RENDERER_RESOURCE_BYTES",
                            format!("content/{}", artifact.path),
                            "renderer mesh resource byte accounting overflowed",
                        )
                    })?;
                    if mesh_count > RENDERER_MESH_MAX_COUNT
                        || mesh_bytes > RENDERER_MESH_MAX_TOTAL_BYTES
                    {
                        return Err(ProductAssemblyError::new(
                            "ASSEMBLY_RENDERER_RESOURCE_BOUNDS",
                            format!("content/{}", artifact.path),
                            "renderer mesh resource count or aggregate bytes exceed the Product Assembly bound",
                        ));
                    }
                }
                _ => unreachable!("fixed renderer preload kind"),
            }
            let hash = artifact
                .content_hash
                .expect("load-required content has identity")
                .to_hex();
            let identity = format!("{kind}-resource/{hash}");
            if let Some(first_path) =
                renderer_identities.insert(identity.clone(), artifact.path.clone())
            {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_RENDERER_RESOURCE_IDENTITY_DUPLICATE",
                    format!("content/{}", artifact.path),
                    format!("renderer preload identity {identity} duplicates content/{first_path}"),
                ));
            }
            renderer_preload.push(RendererPreloadResource {
                identity,
                content_hash: format!("sha256:{hash}"),
                media_type: match kind {
                    "texture" => "image/png",
                    "audio" => "audio/wav",
                    "mesh" => "application/octet-stream",
                    _ => unreachable!("fixed renderer preload kind"),
                },
                path: format!("content/{}", artifact.path),
                byte_length: file.bytes.len(),
            });
        }
        runtime_files.push(file.clone());
    }
    runtime_files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    runtime_json_content.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    renderer_preload.sort_by(|left, right| left.identity.cmp(&right.identity));
    Ok(ContentClosure {
        manifest_source: Some(source_manifest),
        runtime_files,
        runtime_json_content,
        renderer_preload,
    })
}

pub(crate) fn validate_renderer_preload_resource(
    kind: &str,
    path: &str,
    bytes: &[u8],
) -> Result<(), ProductAssemblyError> {
    if !content_store::is_safe_relative_path(path) || path.contains('%') {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RENDERER_RESOURCE_PATH",
            format!("content/{path}"),
            "renderer preload resources must retain a safe non-percent-aliased content-manifest relative path",
        ));
    }
    let host_path = format!("content/{path}");
    if host_path.len() > 512
        || !host_path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
    {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RENDERER_RESOURCE_PATH",
            &host_path,
            "renderer preload paths must use the fixed generated loopback host path alphabet",
        ));
    }
    let (extension, media_type, minimum, maximum, signature) = match kind {
        "texture" => (
            "png",
            "image/png",
            8,
            RENDERER_TEXTURE_MAX_BYTES,
            bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        ),
        "audio" => (
            "wav",
            "audio/wav",
            44,
            RENDERER_AUDIO_MAX_BYTES,
            bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        ),
        "mesh" => (
            "rmesh",
            "application/octet-stream",
            render_model::MESH_RESOURCE_HEADER_BYTES as usize,
            RENDERER_MESH_MAX_BYTES,
            render_model::validate_mesh_resource_header(bytes).is_ok(),
        ),
        _ => unreachable!("fixed renderer preload kind"),
    };
    if !path.ends_with(&format!(".{extension}")) {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RENDERER_RESOURCE_MEDIA",
            format!("content/{path}"),
            format!("{kind} renderer resources must use {media_type} {extension} files"),
        ));
    }
    if bytes.len() < minimum || bytes.len() > maximum || !signature {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_RENDERER_RESOURCE_MEDIA",
            format!("content/{path}"),
            format!("{kind} renderer resource bytes do not match bounded {media_type} media"),
        ));
    }
    Ok(())
}

fn render_renderer_preload(
    resources: &[RendererPreloadResource],
) -> Result<Vec<u8>, ProductAssemblyError> {
    serde_json::to_vec(&RendererPreloadDescriptor {
        artifact: "rusty.product.renderer-preload.v1",
        resources,
    })
    .map_err(|error| {
        ProductAssemblyError::new(
            "ASSEMBLY_RENDERER_PRELOAD",
            BROWSER_RENDERER_PRELOAD_PATH,
            error.to_string(),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn generate_source_plan(
    manifest: &ProductManifest,
    kernel_source: Option<&KernelSource>,
    canonical_composition: &[u8],
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
    engine_dependency_path: &str,
    kernel_dependency_path: Option<&str>,
    browser_files: &[PublicationFile],
    runtime_files: &[ProductFile],
    runtime_json_content: &[ProductFile],
    renderer_preload: &[RendererPreloadResource],
    runtime_program: Option<&[u8]>,
) -> Result<AssemblySourcePlan, ProductAssemblyError> {
    let name = format!(
        "rusty-product-{}",
        manifest
            .product_id()
            .chars()
            .map(|character| if character == '.' || character == '_' {
                '-'
            } else {
                character
            })
            .collect::<String>()
    );
    let engine_dependency = format!(
        "rusty-engine = {{ path = {} }}",
        rust_string(engine_dependency_path)
    );
    let kernel_dependency = kernel_dependency_path.map_or_else(String::new, |path| {
        format!("\nproduct-kernel = {{ path = {} }}", rust_string(path))
    });
    let package_dependency = match kernel_source {
        Some(KernelSource::Package(package)) => format!(
            "\nproduct-runtime = {{ package = {}, path = \"kernel\" }}",
            rust_string(&package.package_name),
        ),
        _ => String::new(),
    };
    let cargo = format!(
        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[lib]\nname = \"rusty_product\"\npath = \"src/lib.rs\"\n\n[dependencies]\n{engine_dependency}{kernel_dependency}{package_dependency}\nserde_json = \"1\"\n\n# Dependencies are product-relative paths supplied by the assembly caller; no absolute path is embedded.\n\n# This generated package is a detached build root even when its product lives below an Engine checkout.\n[workspace]\n",
    )
    .into_bytes();
    let kernel = match kernel_source {
        Some(KernelSource::Entry(path)) => Some(path.as_str().to_owned()),
        _ => None,
    };
    if let Some(path) = &kernel {
        if !path.ends_with(".rs")
            || !path.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-')
            })
        {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_KERNEL_SOURCE",
                path,
                "the source-linked Product Kernel entry must be a Rust source file",
            ));
        }
    }
    let main = "#![forbid(unsafe_code)]\n\nfn main() {\n    let mut port = 0_u16;\n    let mut arguments = std::env::args().skip(1);\n    while let Some(argument) = arguments.next() {\n        if argument != \"--port\" {\n            eprintln!(\"unsupported generated Product option: {argument}\");\n            std::process::exit(2);\n        }\n        let Some(value) = arguments.next() else {\n            eprintln!(\"--port requires a bounded u16 value\");\n            std::process::exit(2);\n        };\n        port = value.parse().unwrap_or_else(|_| {\n            eprintln!(\"--port requires a bounded u16 value\");\n            std::process::exit(2);\n        });\n    }\n    rusty_product::product::run(port);\n}\n";
    let lib = if let Some(path) = &kernel {
        format!(
            "#![forbid(unsafe_code)]\n\n#[allow(dead_code)]\n#[path = \"../source/{path}\"]\nmod product_kernel_source;\n\npub mod product;\n"
        )
    } else if matches!(kernel_source, Some(KernelSource::Package(_))) {
        "#![forbid(unsafe_code)]\n\npub use product_runtime as product_kernel_package;\n\npub mod product;\n".to_owned()
    } else {
        "#![forbid(unsafe_code)]\n\npub mod product;\n".to_owned()
    };
    let kernel_descriptors = render_kernel_descriptors(kernel_capabilities);
    let bundle_entries = render_bundle_entries(browser_files, runtime_files, renderer_preload)?;
    let runtime_resources = render_runtime_resources(browser_files, runtime_files)?;
    let product = if runtime_program.is_some() {
        render_vm_runtime_product_source(
            &bundle_entries,
            runtime_json_content,
            manifest.ui_projection_stream().unwrap_or("product.runtime"),
            manifest
                .ui_projection_contract()
                .unwrap_or("rusty.product.runtime-projection.v1"),
        )
    } else {
        render_runtime_product_source(
            &bundle_entries,
            &kernel_descriptors,
            &runtime_resources,
            kernel_source.is_some(),
            matches!(kernel_source, Some(KernelSource::Package(_))),
        )
    };
    let lib_bytes = lib.into_bytes();
    let main_bytes = main.as_bytes().to_vec();
    let product_bytes = product.into_bytes();
    let generated = [&cargo, &lib_bytes, &main_bytes, &product_bytes];
    if generated
        .iter()
        .any(|bytes| bytes.len() > MAX_GENERATED_SOURCE_BYTES)
    {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_GENERATED_SOURCE_BYTES",
            "product-assembly/src",
            format!("generated source files are limited to {MAX_GENERATED_SOURCE_BYTES} bytes"),
        ));
    }
    let _ = canonical_composition;
    Ok(AssemblySourcePlan {
        cargo_toml: cargo,
        lib_rs: lib_bytes,
        main_rs: main_bytes,
        product_rs: product_bytes,
        kernel_entry: kernel,
        kernel_package: kernel_source.and_then(|source| match source {
            KernelSource::Package(package) => Some(package.manifest_path.to_string()),
            KernelSource::Entry(_) => None,
        }),
        runtime_program_path: runtime_program.map(|_| VM_RUNTIME_PROGRAM_PATH.to_owned()),
        compiled_composition_path: "artifacts/compiled-composition.json".to_owned(),
    })
}

fn render_kernel_descriptors(descriptors: &[ProductKernelCapabilityDescriptor]) -> String {
    descriptors
        .iter()
        .map(|descriptor| {
            let metadata = descriptor.metadata();
            format!(
                "product_model::ProductKernelCapabilityDescriptor::new({}, product_model::CapabilityMetadata::new(product_model::CapabilityKind::{}, {}, {}, product_model::CapabilityAccess::new({}, {}), product_model::CapabilityBudget::new({}), product_model::CapabilityProvenance::new({}, {}, {}))),",
                rust_string(descriptor.identity()),
                render_kind(metadata.kind()),
                render_uses(metadata.uses()),
                render_availability(metadata.availability()),
                render_str_slice(metadata.access().reads()),
                render_str_slice(metadata.access().writes()),
                metadata.budget().maximum_compact_json_payload_bytes(),
                rust_string(metadata.provenance().owner()),
                rust_string(metadata.provenance().source()),
                rust_string(metadata.provenance().logical_path()),
            )
        })
        .collect()
}

/// Renders the already-admitted browser closure into generated Rust source.
/// The generated package only reaches sibling bundle bytes through
/// `include_bytes!`; it never performs a runtime filesystem read.
fn render_bundle_entries(
    browser_files: &[PublicationFile],
    runtime_files: &[ProductFile],
    renderer_preload: &[RendererPreloadResource],
) -> Result<String, ProductAssemblyError> {
    let mut files = browser_files
        .iter()
        .map(|file| (file.relative_path().as_str(), file.relative_path().as_str()))
        .collect::<Vec<_>>();
    let runtime_by_path = runtime_files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    for resource in renderer_preload {
        let file = runtime_by_path.get(resource.path.as_str()).ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_RENDERER_PRELOAD_MISSING",
                &resource.path,
                "renderer preload descriptor path has no admitted runtime content body",
            )
        })?;
        let path = file.relative_path.as_str();
        files.push((path, path));
    }
    if files.len() > product_dev_host::MAX_BUNDLE_ENTRIES {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_FILE_COUNT",
            "product-bundle",
            format!(
                "generated host bundle is limited to {} files",
                product_dev_host::MAX_BUNDLE_ENTRIES
            ),
        ));
    }
    let mut paths = BTreeSet::new();
    files.sort_unstable_by(|left, right| left.0.cmp(right.0));
    files
        .into_iter()
        .map(|(path, include_path)| {
            if !paths.insert(path) {
                return Err(ProductAssemblyError::new(
                    "ASSEMBLY_BROWSER_DUPLICATE_FILE",
                    path,
                    "generated host bundle entries must have unique paths",
                ));
            }
            let content_type = browser_content_type(path).ok_or_else(|| {
                ProductAssemblyError::new(
                    "ASSEMBLY_BROWSER_CONTENT_TYPE",
                    path,
                    "browser bundle resource has no admitted generated content type",
                )
            })?;
            Ok(format!(
                "product_dev_host::ProductDevBundleEntry::new({}, {}, include_bytes!(\"../../product-bundle/{}\").to_vec()).expect(\"generated Product Bundle entry remains admitted\"),",
                rust_string(path),
                rust_string(content_type),
                include_path,
            ))
        })
        .collect::<Result<Vec<_>, ProductAssemblyError>>()
        .map(|entries| entries.concat())
}

/// Renders the immutable generated resource closure supplied to a source-linked
/// `RustyProductRuntime` definition.  These are the same bundle bytes served
/// by the generated host; the runtime receives no product-root path and never
/// reopens the generated tree.
fn render_runtime_resources(
    browser_files: &[PublicationFile],
    runtime_files: &[ProductFile],
) -> Result<String, ProductAssemblyError> {
    let mut entries = Vec::with_capacity(browser_files.len() + runtime_files.len());
    for file in browser_files {
        let path = file.relative_path().as_str();
        entries.push(format!(
            "product_kernel::ProductRuntimeResource::new({}, include_bytes!(\"../../product-bundle/{}\")),",
            rust_string(path), path
        ));
    }
    for file in runtime_files {
        let path = file.relative_path.as_str();
        entries.push(format!(
            "product_kernel::ProductRuntimeResource::new({}, include_bytes!(\"../../product-bundle/{}\")),",
            rust_string(path), path
        ));
    }
    Ok(entries.concat())
}

/// Emits the generated runtime owner for the Engine-owned JavaScript VM lane.
/// It deliberately bypasses the legacy Product Kernel adapter/composition
/// path: lifecycle admission, input snapshots, VM state, and UI publication
/// remain visible, direct named owners in the generated product.
fn render_vm_runtime_product_source(
    bundle_entries: &str,
    runtime_json_content: &[ProductFile],
    projection_stream: &str,
    projection_contract: &str,
) -> String {
    r#"use std::fmt::Debug;

use rusty_engine::{
    product_dev_host, product_model, render_model, runtime_input, runtime_lifecycle, runtime_ui,
    runtime_vm,
};

const ADMITTED_COMPILED_COMPOSITION: &[u8] =
    include_bytes!("../artifacts/compiled-composition.json");
const RUNTIME_PROGRAM: &str = include_str!("../runtime/main.js");
const ADMITTED_RUNTIME_JSON_CONTENT: &[(&str, &[u8])] =
    &[__VM_RUNTIME_JSON_CONTENT__];
const VM_PROJECTION_STREAM: &str = __VM_PROJECTION_STREAM__;
const VM_PROJECTION_CONTRACT: &str = __VM_PROJECTION_CONTRACT__;

/// The generated VM runtime owns only its lifecycle, input lane, VM root, and
/// UI projection channel. It does not instantiate ProductRuntimeAdapter or
/// RuntimeComposition, and it has no schedule/mutation/kernel dispatch path.
pub struct GeneratedProductDevRuntime {
    lifecycle: runtime_lifecycle::RuntimeLifecycle,
    mappings: runtime_input::CompiledInputMappings,
    input: Option<runtime_input::RuntimeInputLane>,
    projection: Option<runtime_ui::RuntimeUiProjection>,
    vm: Option<runtime_vm::RuntimeVm>,
}

fn initialization_facts() -> Result<serde_json::Value, String> {
    let composition: serde_json::Value = serde_json::from_slice(ADMITTED_COMPILED_COMPOSITION)
        .map_err(|error| format!("compiled composition facts: {error}"))?;
    let content = ADMITTED_RUNTIME_JSON_CONTENT
        .iter()
        .map(|(path, bytes)| {
            let value: serde_json::Value = serde_json::from_slice(bytes)
                .map_err(|error| format!("runtime JSON facts {path}: {error}"))?;
            Ok(serde_json::json!({ "path": path, "value": value }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::json!({
        "composition": composition,
        "content": content,
    }))
}

fn runtime_lifecycle_config(
    manifest: &product_model::ProductManifest,
) -> Result<runtime_lifecycle::RuntimeLifecycleConfig, String> {
    match manifest.lifecycle() {
        product_model::LifecycleMode::Realtime => {
            let realtime = manifest
                .realtime()
                .ok_or_else(|| "manifest realtime settings are missing".to_owned())?;
            runtime_lifecycle::RealtimeLifecycleConfig::new(
                realtime.fixed_step_hz(),
                realtime.max_catch_up_steps(),
            )
            .map(runtime_lifecycle::RuntimeLifecycleConfig::Realtime)
            .map_err(|error| format!("lifecycle configuration: {error}"))
        }
        product_model::LifecycleMode::Demand => Ok(runtime_lifecycle::RuntimeLifecycleConfig::Demand),
        product_model::LifecycleMode::External => Ok(runtime_lifecycle::RuntimeLifecycleConfig::External),
    }
}

impl GeneratedProductDevRuntime {
    pub fn new() -> Result<Self, String> {
        let manifest = product_model::decode_product_manifest(include_str!("../source/rusty.toml"))
            .map_err(|error| format!("manifest admission: {error}"))?;
        let composition = product_model::decode_compiled_composition(ADMITTED_COMPILED_COMPOSITION)
            .map_err(|error| format!("compiled composition admission: {error}"))?;
        let admitted = product_model::admit_checked_product_composition(&manifest, composition)
            .map_err(|error| format!("composition admission: {error}"))?;
        let linked = product_model::link_admitted_product_composition(admitted, &[])
            .map_err(|error| format!("composition linkage: {error}"))?;
        let mappings = runtime_input::CompiledInputMappings::compile(&linked)
            .map_err(|error| format!("input compilation: {error}"))?;
        let config = runtime_lifecycle_config(&manifest)?;
        Ok(Self {
            lifecycle: runtime_lifecycle::RuntimeLifecycle::new(
                runtime_lifecycle::RuntimeInstanceId::new(1),
                config,
            ),
            mappings,
            input: None,
            projection: None,
            vm: None,
        })
    }

    fn initialize_vm(&mut self) -> Result<(), String> {
        let mut vm = runtime_vm::RuntimeVm::new(RUNTIME_PROGRAM, runtime_vm::RuntimeVmConfig::default())
            .map_err(|error| format!("VM configuration: {error}"))?;
        vm.initialize(initialization_facts()?)
            .map_err(|error| format!("VM initialization: {error}"))?;
        self.vm = Some(vm);
        Ok(())
    }

    fn bind_epoch(&mut self) -> Result<(), String> {
        let binding = runtime_input::RuntimeInputBinding::new(
            self.lifecycle.instance_id(),
            self.lifecycle.generation(),
            self.lifecycle.control_revision(),
        );
        let context = runtime_input::InputContext::new("gameplay.default")
            .map_err(|error| format!("input context: {error:?}"))?;
        let input = runtime_input::RuntimeInputLane::new(self.mappings.clone(), binding, context);
        // Recreating on each control epoch makes pending host facts incapable
        // of crossing pause/resume/restart ownership boundaries.
        if let Some(previous) = self.input.take() {
            let mut previous = previous;
            previous.dispose();
        }
        self.input = Some(input);
        self.projection = Some(
            runtime_ui::RuntimeUiProjection::bind(&self.lifecycle)
                .map_err(|error| format!("UI projection bind: {error}"))?,
        );
        Ok(())
    }

    fn binding(&self) -> product_dev_host::ProductDevRuntimeBinding {
        let readout = self.lifecycle.readout();
        product_dev_host::ProductDevRuntimeBinding {
            instance_id: product_dev_host::CanonicalU64::new(readout.instance_id().value()),
            generation: product_dev_host::CanonicalU64::new(readout.generation().value()),
            control_revision: product_dev_host::CanonicalU64::new(readout.control_revision().value()),
        }
    }

    fn readout(&self) -> product_dev_host::ProductDevRuntimeReadout {
        let value = self.lifecycle.readout();
        let mode = match value.mode() {
            runtime_lifecycle::RuntimeMode::Realtime => product_dev_host::ProductDevRuntimeMode::Realtime,
            runtime_lifecycle::RuntimeMode::Demand => product_dev_host::ProductDevRuntimeMode::Demand,
            runtime_lifecycle::RuntimeMode::External => product_dev_host::ProductDevRuntimeMode::External,
        };
        let state = match value.state() {
            runtime_lifecycle::RuntimeState::Created => product_dev_host::ProductDevRuntimeState::Created,
            runtime_lifecycle::RuntimeState::Running => product_dev_host::ProductDevRuntimeState::Running,
            runtime_lifecycle::RuntimeState::Paused => product_dev_host::ProductDevRuntimeState::Paused,
            runtime_lifecycle::RuntimeState::Faulted => product_dev_host::ProductDevRuntimeState::Faulted,
            runtime_lifecycle::RuntimeState::Shutdown => product_dev_host::ProductDevRuntimeState::Shutdown,
        };
        let mut readout = product_dev_host::ProductDevRuntimeReadout::new(self.binding(), mode, state)
            .with_counters(
                value.admitted_simulation_steps(),
                value.admitted_presentations(),
                value.dropped_realtime_steps().min(u64::MAX as u128) as u64,
                value.clock_regressions(),
            )
            .with_clock(value.scaled_remainder(), value.last_observed_time().map(|time| time.nanoseconds()));
        if let Some(fault) = value.fault() {
            readout = readout.with_fault(match fault {
                runtime_lifecycle::RuntimeFault::OwnerReported => product_dev_host::ProductDevRuntimeFault::OwnerReported,
                runtime_lifecycle::RuntimeFault::CounterExhausted => product_dev_host::ProductDevRuntimeFault::CounterExhausted,
            });
        }
        readout
    }

    fn runtime_error(&self, code: &str, detail: impl Debug) -> product_dev_host::ProductDevRuntimeError {
        let mut diagnostic = format!("{detail:?}");
        if diagnostic.len() > 1_024 {
            let mut end = 1_024;
            while !diagnostic.is_char_boundary(end) { end -= 1; }
            diagnostic.truncate(end);
        }
        product_dev_host::ProductDevRuntimeError::new(code, diagnostic)
            .unwrap_or_else(|_| product_dev_host::ProductDevRuntimeError::new("GENERATED_RUNTIME_ERROR", "generated runtime diagnostic rejected").expect("fixed diagnostic"))
    }

    fn receipt<T>(
        &self,
        result: T,
        ui: Vec<runtime_ui::RuntimeUiProjectionEnvelope>,
        render: Vec<render_model::RenderFrameDiff>,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<T>, product_dev_host::ProductDevRuntimeError> {
        let mut outputs = Vec::with_capacity(2 + ui.len() + render.len());
        outputs.push(product_dev_host::ProductDevRuntimeOutput::binding(self.binding()));
        for envelope in &ui {
            outputs.push(product_dev_host::ProductDevRuntimeOutput::ui_projection(envelope)
                .map_err(|error| self.runtime_error("VM_UI_OUTPUT", error))?);
        }
        for frame in &render {
            outputs.push(product_dev_host::ProductDevRuntimeOutput::frame(frame)
                .map_err(|error| self.runtime_error("VM_RENDER_OUTPUT", error))?);
        }
        outputs.push(product_dev_host::ProductDevRuntimeOutput::runtime_readout(self.readout()));
        product_dev_host::ProductDevRuntimeReceipt::new(result, outputs)
            .map_err(|error| self.runtime_error("VM_RECEIPT", error))
    }

    fn accepted_operation(
        &self,
        operation: product_dev_host::ProductDevOperationKind,
    ) -> Result<product_dev_host::ProductDevOperationResult, product_dev_host::ProductDevRuntimeError> {
        product_dev_host::ProductDevOperationResult::accepted(operation, self.binding(), self.readout())
            .map_err(|error| self.runtime_error("VM_OPERATION_RESULT", error))
    }

    fn rejected_operation(
        &self,
        operation: product_dev_host::ProductDevOperationKind,
        error: impl Debug,
    ) -> Result<product_dev_host::ProductDevOperationResult, product_dev_host::ProductDevRuntimeError> {
        product_dev_host::ProductDevOperationResult::rejected(operation, format!("{error:?}"))
            .map_err(|host_error| self.runtime_error("VM_OPERATION_RESULT", host_error))
    }

    fn run_admission(
        &mut self,
        admission: runtime_lifecycle::SimulationAdmission,
    ) -> Result<(Vec<runtime_ui::RuntimeUiProjectionEnvelope>, Vec<render_model::RenderFrameDiff>), String> {
        let mut emitted = Vec::new();
        let mut render = Vec::new();
        for offset in 0..admission.step_count() {
            let step = admission.step_at(offset).ok_or("admitted simulation step disappeared")?;
            let phases = step.phases();
            let (_, intents) = self.input.as_mut().ok_or("input lane is not active")?
                .snapshot_for_step(&self.lifecycle, phases.input_snapshot())
                .map_err(|error| format!("input snapshot: {error:?}"))?;
            let input = serde_json::json!({
                "intents": intents.iter().map(encode_intent).collect::<Vec<_>>(),
            });
            let candidate = self.vm.as_ref().ok_or("VM is not active")?
                .prepare_turn(runtime_vm::RuntimeTurn {
                    step: serde_json::json!({ "sequence": step.token().step().value() }),
                    input,
                })
                .map_err(|error| format!("VM turn: {error}"))?;
            let prepared_ui = self.projection.as_ref().ok_or("UI projection lane is not active")?
                .prepare_value(
                    &self.lifecycle,
                    phases.projection(),
                    VM_PROJECTION_STREAM,
                    VM_PROJECTION_CONTRACT,
                    candidate.receipt().projection().ui().clone(),
                )
                .map_err(|error| format!("UI projection: {error}"))?;
            let frame = if let Some(value) = candidate.receipt().projection().render() {
                let encoded = serde_json::to_string(value)
                    .map_err(|error| format!("VM render encoding: {error}"))?;
                Some(render_model::RenderFrameDiff::decode_json(&encoded)
                    .map_err(|error| format!("VM render decode: {error}"))?)
            } else {
                None
            };
            let envelope = self.projection.as_mut().ok_or("UI projection lane is not active")?
                .commit_prepared(&self.lifecycle, prepared_ui)
                .map_err(|error| format!("UI projection: {error}"))?;
            self.vm.as_mut().ok_or("VM is not active")?
                .commit_prepared(candidate)
                .map_err(|error| format!("VM commit: {error}"))?;
            emitted.push(envelope);
            if let Some(frame) = frame { render.push(frame); }
        }
        Ok((emitted, render))
    }

    fn bundle() -> Result<product_dev_host::ProductDevBundle, String> {
        product_dev_host::ProductDevBundle::new(vec![__BUNDLE_ENTRIES__])
            .map_err(|error| error.to_string())
    }
}

fn encode_intent(intent: &runtime_input::RuntimeIntentEnvelope) -> serde_json::Value {
    let value = match intent.value() {
        runtime_input::RuntimeIntentValue::Digital { active } => serde_json::json!({ "kind": "digital", "active": active }),
        runtime_input::RuntimeIntentValue::Axis { value } => serde_json::json!({ "kind": "axis", "value": value.value() }),
        runtime_input::RuntimeIntentValue::ProductPayload { payload } => serde_json::json!({ "kind": "product-payload", "contract": payload.contract(), "data": payload.data() }),
    };
    let phase = match intent.phase() {
        runtime_input::IntentPhase::Held => "held",
        runtime_input::IntentPhase::Pressed => "pressed",
        runtime_input::IntentPhase::Released => "released",
        runtime_input::IntentPhase::Axis => "axis",
        runtime_input::IntentPhase::DirectUi => "direct-ui",
    };
    serde_json::json!({
        "id": intent.intent(),
        "sequence": intent.sequence(),
        "phase": phase,
        "value": value,
    })
}

impl product_dev_host::ProductDevRuntime for GeneratedProductDevRuntime {
    fn lifecycle(
        &mut self,
        operation: product_dev_host::ProductDevLifecycleOperation,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let transition = match operation {
            product_dev_host::ProductDevLifecycleOperation::Start => self.lifecycle.start().map(|_| ()),
            product_dev_host::ProductDevLifecycleOperation::Pause => self.lifecycle.pause().map(|_| ()),
            product_dev_host::ProductDevLifecycleOperation::Resume => self.lifecycle.resume().map(|_| ()),
            product_dev_host::ProductDevLifecycleOperation::Restart => self.lifecycle.restart().map(|_| ()),
            product_dev_host::ProductDevLifecycleOperation::Shutdown => self.lifecycle.shutdown().map(|_| ()),
            product_dev_host::ProductDevLifecycleOperation::ReportFault => self.lifecycle.report_fault(runtime_lifecycle::RuntimeFault::OwnerReported).map(|_| ()),
        };
        match transition {
            Ok(()) => {},
            Err(error) => return self.receipt(self.rejected_operation(operation.operation_kind(), error)?, Vec::new(), Vec::new()),
        }
        match operation {
            product_dev_host::ProductDevLifecycleOperation::Start
            | product_dev_host::ProductDevLifecycleOperation::Restart => {
                if let Err(error) = self.initialize_vm().and_then(|_| self.bind_epoch()) {
                    let _ = self.lifecycle.report_fault(runtime_lifecycle::RuntimeFault::OwnerReported);
                    return self.receipt(self.rejected_operation(operation.operation_kind(), error)?, Vec::new(), Vec::new());
                }
            }
            product_dev_host::ProductDevLifecycleOperation::Resume => {
                if let Err(error) = self.bind_epoch() {
                    let _ = self.lifecycle.report_fault(runtime_lifecycle::RuntimeFault::OwnerReported);
                    return self.receipt(self.rejected_operation(operation.operation_kind(), error)?, Vec::new(), Vec::new());
                }
            }
            product_dev_host::ProductDevLifecycleOperation::Pause => {
                if let Some(input) = self.input.as_mut() { input.dispose(); }
                self.input = None;
                self.projection = None;
            }
            product_dev_host::ProductDevLifecycleOperation::Shutdown
            | product_dev_host::ProductDevLifecycleOperation::ReportFault => {
                if let Some(input) = self.input.as_mut() { input.dispose(); }
                self.input = None;
                self.projection = None;
                self.vm = None;
            }
        }
        self.receipt(self.accepted_operation(operation.operation_kind())?, Vec::new(), Vec::new())
    }

    fn input(
        &mut self,
        batch: product_dev_host::ProductDevInputBatch,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevInputResult>, product_dev_host::ProductDevRuntimeError> {
        let count = batch.events().len();
        let result = match self.input.as_mut() {
            Some(lane) => batch.events().iter().cloned().try_for_each(|event| {
                lane.ingest(event).map_err(|error| format!("{error:?}"))
            }),
            None => Err("input lane is not active".to_owned()),
        };
        let result = match result {
            Ok(()) => product_dev_host::ProductDevInputResult::accepted(count, self.binding(), self.readout()),
            Err(error) => product_dev_host::ProductDevInputResult::rejected(error),
        }.map_err(|error| self.runtime_error("VM_INPUT_RESULT", error))?;
        self.receipt(result, Vec::new(), Vec::new())
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: product_dev_host::CanonicalU64,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let admission = self.lifecycle.advance_realtime(runtime_lifecycle::HostMonotonicTime::from_nanoseconds(observed_time_ns.get()));
        let admission = match admission {
            Ok(value) => value,
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdvanceRealtime, error)?, Vec::new(), Vec::new()),
        };
        let (ui, render) = match admission.simulation().map(|value| self.run_admission(value)).transpose() {
            Ok(Some(value)) => value,
            Ok(None) => (Vec::new(), Vec::new()),
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdvanceRealtime, error)?, Vec::new(), Vec::new()),
        };
        self.receipt(self.accepted_operation(product_dev_host::ProductDevOperationKind::AdvanceRealtime)?, ui, render)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let admission = match self.lifecycle.admit_demand_step() {
            Ok(value) => value,
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitDemandStep, error)?, Vec::new(), Vec::new()),
        };
        let (ui, render) = match self.run_admission(admission) {
            Ok(value) => value,
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitDemandStep, error)?, Vec::new(), Vec::new()),
        };
        self.receipt(self.accepted_operation(product_dev_host::ProductDevOperationKind::AdmitDemandStep)?, ui, render)
    }

    fn admit_external_step(
        &mut self,
        step: product_dev_host::CanonicalU64,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let admission = match self.lifecycle.admit_external_step(runtime_lifecycle::ExternalStep::new(step.get())) {
            Ok(value) => value,
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitExternalStep, error)?, Vec::new(), Vec::new()),
        };
        let (ui, render) = match self.run_admission(admission) {
            Ok(value) => value,
            Err(error) => return self.receipt(self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitExternalStep, error)?, Vec::new(), Vec::new()),
        };
        self.receipt(self.accepted_operation(product_dev_host::ProductDevOperationKind::AdmitExternalStep)?, ui, render)
    }

    fn complete_timeline(
        &mut self,
        completion: product_dev_host::ProductDevTimelineCompletion,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevTimelineCompletionResult>, product_dev_host::ProductDevRuntimeError> {
        let result = product_dev_host::ProductDevTimelineCompletionResult::rejected(
            product_dev_host::CanonicalU64::new(completion.envelope().ticket().value()),
            "the VM runtime does not admit timeline completion",
        ).map_err(|error| self.runtime_error("VM_TIMELINE_RESULT", error))?;
        self.receipt(result, Vec::new(), Vec::new())
    }
}

pub fn product_bundle() -> Result<product_dev_host::ProductDevBundle, String> {
    GeneratedProductDevRuntime::bundle()
}

pub fn start_host(port: u16) -> Result<product_dev_host::RunningProductDevHost, String> {
    let runtime = GeneratedProductDevRuntime::new()?;
    let bundle = product_bundle()?;
    product_dev_host::ProductDevHost::start(runtime, product_dev_host::ProductDevHostConfig::new(port, bundle))
        .map_err(|error| error.to_string())
}

pub fn run(port: u16) {
    let host = start_host(port).expect("generated Product Dev Host starts");
    println!("{}", host.origin());
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    host.shutdown().expect("generated Product Dev Host shuts down");
}
"#
    .replace("__BUNDLE_ENTRIES__", bundle_entries)
    .replace("__VM_PROJECTION_STREAM__", &rust_string(projection_stream))
    .replace("__VM_PROJECTION_CONTRACT__", &rust_string(projection_contract))
    .replace(
        "__VM_RUNTIME_JSON_CONTENT__",
        &render_vm_runtime_json_content(runtime_json_content),
    )
}

fn render_vm_runtime_json_content(runtime_json_content: &[ProductFile]) -> String {
    runtime_json_content
        .iter()
        .map(|file| {
            let path = file.relative_path.as_str();
            format!(
                "({}, include_bytes!({})),",
                rust_string(path),
                rust_string(&format!("../{path}")),
            )
        })
        .collect()
}

/// Emits the Product Assembly executable and its fixed local host bridge. The
/// no-kernel path owns one concrete EmptyProductAdapter only for a composition
/// with no capability bindings. A kernel composition instead links the fixed
/// source symbol `RustyProductRuntime`, builds its concrete adapter from
/// immutable generated resources, and places that adapter in the same
/// host-neutral RuntimeComposition. Runtime operations cross the host through
/// typed ProductDevRuntime DTOs; no runtime filesystem read or generic invoke
/// surface is generated.
fn render_runtime_product_source(
    bundle_entries: &str,
    kernel_descriptors: &str,
    runtime_resources: &str,
    has_kernel: bool,
    package_kernel: bool,
) -> String {
    let runtime_definition = if package_kernel {
        "product_runtime::RustyProductRuntime"
    } else {
        "crate::product_kernel_source::RustyProductRuntime"
    };
    let adapter_type = if has_kernel {
        format!("<{runtime_definition} as product_kernel::ProductKernelRuntimeDefinition>::Adapter")
    } else {
        "EmptyProductAdapter".to_owned()
    };
    let adapter_construction = if has_kernel {
        r#"        validate_kernel_definition()?;
        let resources = product_kernel::ProductRuntimeResources::new(
            ADMITTED_COMPILED_COMPOSITION,
            PRODUCT_RUNTIME_RESOURCES,
        );
        let mut adapter =
            <__RUNTIME_DEFINITION__ as product_kernel::ProductKernelRuntimeDefinition>::build(resources)
                .map_err(|error| format!("Product Runtime definition build: {error:?}"))?;
        <__RUNTIME_DEFINITION__ as product_kernel::ProductKernelRuntimeDefinition>::bind_standard_capabilities(
            &mut adapter,
            standard_capabilities,
        )
        .map_err(|error| format!("Product Runtime definition standard capability binding: {error}"))?;
"#
        .replace("__RUNTIME_DEFINITION__", runtime_definition)
    } else {
        "        let adapter = EmptyProductAdapter::new();\n".to_owned()
    };
    let kernel_validation = if has_kernel {
        r#"fn validate_kernel_definition() -> Result<(), String> {
    let capabilities =
        <__RUNTIME_DEFINITION__ as product_kernel::ProductKernelRuntimeDefinition>::capabilities();
    if capabilities != PRODUCT_KERNEL_CAPABILITIES {
        return Err("Product Runtime definition capabilities differ from the admitted Product Model slice".to_owned());
    }
    let selections =
        <__RUNTIME_DEFINITION__ as product_kernel::ProductKernelRuntimeDefinition>::selections();
    if selections.len() != capabilities.len() {
        return Err("Product Runtime definition selections do not cover every Product Kernel capability".to_owned());
    }
    for (selection_index, selection) in selections.iter().enumerate() {
        if selection.target() != format!("kernel.{}", selection.identity())
            || selection.contract_type().is_empty()
        {
            return Err(format!(
                "invalid Product Runtime definition selection {}",
                selection.identity()
            ));
        }
        if selections[..selection_index]
            .iter()
            .any(|candidate| candidate.identity() == selection.identity())
        {
            return Err(format!(
                "duplicate Product Runtime definition selection {}",
                selection.identity()
            ));
        }
        let Some(capability) = capabilities
            .iter()
            .find(|capability| capability.identity() == selection.identity())
        else {
            return Err(format!(
                "Product Runtime definition selection {} has no capability descriptor",
                selection.identity()
            ));
        };
        if capability.metadata().kind() != selection.kind() {
            return Err(format!(
                "Product Runtime definition selection {} has a kind mismatch",
                selection.identity()
            ));
        }
    }
    for capability in capabilities {
        if !selections
            .iter()
            .any(|selection| selection.identity() == capability.identity())
        {
            return Err(format!(
                "Product Runtime definition capability {} has no concrete owner selection",
                capability.identity()
            ));
        }
    }
    Ok(())
}

"#
        .replace("__RUNTIME_DEFINITION__", runtime_definition)
    } else {
        String::new()
    };
    let runtime_resource_declaration = if has_kernel {
        "const PRODUCT_RUNTIME_RESOURCES: &[product_kernel::ProductRuntimeResource<'static>] =\n    &[__RUNTIME_RESOURCES__];\n"
    } else {
        ""
    };
    let kernel_import = if has_kernel { "product_kernel, " } else { "" };
    let mutation_catalog = if has_kernel {
        r#"        let mutation_descriptors = <__RUNTIME_DEFINITION__ as product_kernel::ProductKernelRuntimeDefinition>::mutation_descriptors()
            .iter()
            .map(|descriptor| {
                runtime_mutation::MutationCapabilityDescriptor::new(
                    descriptor.binding_id(),
                    descriptor.target(),
                    descriptor.publication_domain(),
                    descriptor.owner(),
                    descriptor.operation_type(),
                )
            })
            .collect::<Vec<_>>();
        let mutation = if mutation_descriptors.is_empty() {
            runtime_mutation::CompiledMutationCatalog::empty()
        } else {
            runtime_mutation::CompiledMutationCatalog::compile(&linked, &mutation_descriptors)
                .map_err(|error| format!("mutation catalog admission: {error:?}"))?
        };
"#
        .replace("__RUNTIME_DEFINITION__", runtime_definition)
    } else {
        "        let mutation = runtime_mutation::CompiledMutationCatalog::empty();\n".to_owned()
    };
    let standard_capabilities = if has_kernel {
        r#"        let schedule = runtime_schedule::CompiledRuntimeSchedule::compile(&linked)
            .map_err(|error| format!("schedule compilation: {error}"))?;
        let standard_capabilities = product_kernel::runtime_standard_capabilities::BoundStandardCapabilities::compile(
            &linked,
            &schedule,
            &mutation,
        )
        .map_err(|error| format!("standard capability compilation: {error}"))?;
"#
    } else {
        "        let schedule = runtime_schedule::CompiledRuntimeSchedule::compile(&linked)\n            .map_err(|error| format!(\"schedule compilation: {error}\"))?;\n"
    };
    let mut source = r#"use std::fmt::Debug;

use rusty_engine::{
    product_dev_host, __KERNEL_IMPORT__product_model, runtime_composition, runtime_input,
    runtime_lifecycle, runtime_mutation, runtime_schedule, runtime_timeline,
};
use runtime_composition::{RuntimeComposition, RuntimeCompositionInputs};

const PRODUCT_KERNEL_CAPABILITIES: &[product_model::ProductKernelCapabilityDescriptor] =
    &[__KERNEL_DESCRIPTORS__];
__RUNTIME_RESOURCE_DECLARATION__
const ADMITTED_COMPILED_COMPOSITION: &[u8] =
    include_bytes!("../artifacts/compiled-composition.json");

fn runtime_lifecycle_config(
    manifest: &product_model::ProductManifest,
) -> Result<runtime_lifecycle::RuntimeLifecycleConfig, String> {
    match manifest.lifecycle() {
        product_model::LifecycleMode::Realtime => {
            let realtime = manifest
                .realtime()
                .ok_or_else(|| "manifest realtime settings are missing".to_owned())?;
            runtime_lifecycle::RealtimeLifecycleConfig::new(
                realtime.fixed_step_hz(),
                realtime.max_catch_up_steps(),
            )
            .map(runtime_lifecycle::RuntimeLifecycleConfig::Realtime)
            .map_err(|error| format!("lifecycle configuration: {error}"))
        }
        product_model::LifecycleMode::Demand => Ok(runtime_lifecycle::RuntimeLifecycleConfig::Demand),
        product_model::LifecycleMode::External => Ok(runtime_lifecycle::RuntimeLifecycleConfig::External),
    }
}

__EMPTY_ADAPTER_START__
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct EmptyProductAuthority;

impl runtime_mutation::MutationAuthority for EmptyProductAuthority {
    type Guard = u64;

    fn guard(&self) -> Self::Guard {
        0
    }

    fn publication_domain(&self) -> &str {
        "engine.empty"
    }
}

#[allow(dead_code)]
#[derive(Debug, Default)]
struct EmptyProductPlanner;

impl runtime_mutation::MutationPlanner<EmptyProductAuthority, ()> for EmptyProductPlanner {
    type Error = String;

    fn stage(
        &mut self,
        _authority: &EmptyProductAuthority,
        _batch: &runtime_mutation::MutationResolvedBatch,
    ) -> Result<runtime_mutation::MutationStage<EmptyProductAuthority, ()>, Self::Error> {
        Err("the Engine empty product has no mutation owner".to_owned())
    }
}

#[allow(dead_code)]
struct EmptyProductAdapter {
    authority: EmptyProductAuthority,
    planner: EmptyProductPlanner,
}

#[allow(dead_code)]
impl EmptyProductAdapter {
    const fn new() -> Self {
        Self {
            authority: EmptyProductAuthority,
            planner: EmptyProductPlanner,
        }
    }
}

impl runtime_composition::ProductRuntimeAdapter for EmptyProductAdapter {
    type Authority = EmptyProductAuthority;
    type Guard = u64;
    type Planner = EmptyProductPlanner;
    type Evidence = ();
    type Error = String;
    type ScheduleOutput = String;
    type UiOutput = String;

    fn on_input(
        &mut self,
        _frame: &runtime_input::InputFrame,
        _intents: &[runtime_input::RuntimeIntentEnvelope],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn dispatch_schedule(
        &mut self,
        invocation: runtime_schedule::ScheduleSystemInvocation<'_>,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::ScheduleOutput, Self::Error> {
        Err(format!(
            "the Engine empty product cannot dispatch authored system `{}`",
            invocation.system_id()
        ))
    }

    fn on_timeline_releases(
        &mut self,
        _releases: &runtime_timeline::TimelineRelease,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn prepare_mutation(
        &mut self,
        _step: runtime_lifecycle::SimulationStep,
    ) -> Result<Option<runtime_mutation::MutationBatch>, Self::Error> {
        Ok(None)
    }

    fn mutation_parts(
        &mut self,
    ) -> (&mut Self::Authority, &mut Self::Planner) {
        (&mut self.authority, &mut self.planner)
    }

    fn project(
        &mut self,
        _lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        _token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<runtime_composition::ProductRuntimeOutputs<Self::UiOutput>, Self::Error> {
        runtime_composition::ProductRuntimeOutputs::new(Vec::new(), None, None)
            .map_err(|error| error.to_string())
    }
}
__EMPTY_ADAPTER_END__

type GeneratedProductAdapter = __ADAPTER_TYPE__;
type GeneratedProductComposition = RuntimeComposition<GeneratedProductAdapter>;
type GeneratedProductStep = runtime_composition::ProductRuntimeStep<GeneratedProductAdapter>;

__KERNEL_VALIDATION__

/// The generated runtime owner is the only value placed behind the
/// development host mutex. All state remains in the host-neutral
/// RuntimeComposition and its product adapter.
pub struct GeneratedProductDevRuntime {
    composition: GeneratedProductComposition,
}

impl GeneratedProductDevRuntime {
    pub fn new() -> Result<Self, String> {
        let manifest = product_model::decode_product_manifest(include_str!("../source/rusty.toml"))
            .map_err(|error| format!("manifest admission: {error}"))?;
        let composition = product_model::decode_compiled_composition(ADMITTED_COMPILED_COMPOSITION)
            .map_err(|error| format!("compiled composition admission: {error}"))?;
        let admitted = product_model::admit_checked_product_composition(&manifest, composition)
            .map_err(|error| format!("composition admission: {error}"))?;
        let linked = product_model::link_admitted_product_composition(
            admitted,
            PRODUCT_KERNEL_CAPABILITIES,
        )
        .map_err(|error| format!("composition linkage: {error}"))?;
__MUTATION_CATALOG__
__STANDARD_CAPABILITIES__
        let inputs = RuntimeCompositionInputs::new(
            runtime_input::CompiledInputMappings::compile(&linked)
                .map_err(|error| format!("input compilation: {error}"))?,
            schedule,
            runtime_timeline::CompiledTimelineCatalog::compile(&linked)
                .map_err(|error| format!("timeline compilation: {error}"))?,
            mutation,
            runtime_input::InputContext::new("gameplay.default")
                .map_err(|error| format!("input context: {error}"))?,
        );
        let config = runtime_lifecycle_config(&manifest)?;
__ADAPTER_CONSTRUCTION__
        Ok(Self {
            composition: RuntimeComposition::new(
                runtime_lifecycle::RuntimeInstanceId::new(1),
                config,
                inputs,
                adapter,
            ),
        })
    }

    fn binding(&self) -> product_dev_host::ProductDevRuntimeBinding {
        let readout = self.composition.lifecycle().readout();
        product_dev_host::ProductDevRuntimeBinding {
            instance_id: product_dev_host::CanonicalU64::new(readout.instance_id().value()),
            generation: product_dev_host::CanonicalU64::new(readout.generation().value()),
            control_revision: product_dev_host::CanonicalU64::new(
                readout.control_revision().value(),
            ),
        }
    }

    fn readout(&self) -> product_dev_host::ProductDevRuntimeReadout {
        let value = self.composition.lifecycle().readout();
        let mode = match value.mode() {
            runtime_lifecycle::RuntimeMode::Realtime => product_dev_host::ProductDevRuntimeMode::Realtime,
            runtime_lifecycle::RuntimeMode::Demand => product_dev_host::ProductDevRuntimeMode::Demand,
            runtime_lifecycle::RuntimeMode::External => product_dev_host::ProductDevRuntimeMode::External,
        };
        let state = match value.state() {
            runtime_lifecycle::RuntimeState::Created => product_dev_host::ProductDevRuntimeState::Created,
            runtime_lifecycle::RuntimeState::Running => product_dev_host::ProductDevRuntimeState::Running,
            runtime_lifecycle::RuntimeState::Paused => product_dev_host::ProductDevRuntimeState::Paused,
            runtime_lifecycle::RuntimeState::Faulted => product_dev_host::ProductDevRuntimeState::Faulted,
            runtime_lifecycle::RuntimeState::Shutdown => product_dev_host::ProductDevRuntimeState::Shutdown,
        };
        let mut readout = product_dev_host::ProductDevRuntimeReadout::new(self.binding(), mode, state)
            .with_counters(
                value.admitted_simulation_steps(),
                value.admitted_presentations(),
                value.dropped_realtime_steps().min(u64::MAX as u128) as u64,
                value.clock_regressions(),
            )
            .with_clock(
                value.scaled_remainder(),
                value.last_observed_time().map(|time| time.nanoseconds()),
            );
        if let Some(fault) = value.fault() {
            readout = readout.with_fault(match fault {
                runtime_lifecycle::RuntimeFault::OwnerReported => product_dev_host::ProductDevRuntimeFault::OwnerReported,
                runtime_lifecycle::RuntimeFault::CounterExhausted => product_dev_host::ProductDevRuntimeFault::CounterExhausted,
            });
        }
        readout
    }

    fn runtime_error(&self, code: &str, detail: impl Debug) -> product_dev_host::ProductDevRuntimeError {
        let mut diagnostic = format!("{detail:?}");
        if diagnostic.len() > 1_024 {
            let mut end = 1_024;
            while !diagnostic.is_char_boundary(end) {
                end -= 1;
            }
            diagnostic.truncate(end);
        }
        product_dev_host::ProductDevRuntimeError::new(code, diagnostic)
            .unwrap_or_else(|_| product_dev_host::ProductDevRuntimeError::new("GENERATED_RUNTIME_ERROR", "generated runtime diagnostic rejected").expect("fixed diagnostic"))
    }

    fn receipt<T>(
        &self,
        result: T,
        steps: &[GeneratedProductStep],
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<T>, product_dev_host::ProductDevRuntimeError> {
        let mut outputs = Vec::with_capacity(2 + steps.len() * 3);
        outputs.push(product_dev_host::ProductDevRuntimeOutput::binding(self.binding()));
        for step in steps {
            for envelope in &step.ui {
                outputs.push(product_dev_host::ProductDevRuntimeOutput::ui_projection(envelope).map_err(|error| self.runtime_error("GENERATED_UI_OUTPUT", error))?);
            }
            if let Some(frame) = &step.render {
                outputs.push(product_dev_host::ProductDevRuntimeOutput::frame(frame).map_err(|error| self.runtime_error("GENERATED_RENDER_OUTPUT", error))?);
            }
            if let Some(frame) = &step.presentation {
                outputs.push(product_dev_host::ProductDevRuntimeOutput::presentation(frame).map_err(|error| self.runtime_error("GENERATED_PRESENTATION_OUTPUT", error))?);
            }
        }
        outputs.push(product_dev_host::ProductDevRuntimeOutput::runtime_readout(self.readout()));
        product_dev_host::ProductDevRuntimeReceipt::new(result, outputs)
            .map_err(|error| self.runtime_error("GENERATED_RECEIPT", error))
    }

    fn accepted_operation(
        &self,
        operation: product_dev_host::ProductDevOperationKind,
    ) -> Result<product_dev_host::ProductDevOperationResult, product_dev_host::ProductDevRuntimeError> {
        product_dev_host::ProductDevOperationResult::accepted(operation, self.binding(), self.readout())
            .map_err(|error| self.runtime_error("GENERATED_OPERATION_RESULT", error))
    }

    fn rejected_operation(
        &self,
        operation: product_dev_host::ProductDevOperationKind,
        error: impl Debug,
    ) -> Result<product_dev_host::ProductDevOperationResult, product_dev_host::ProductDevRuntimeError> {
        product_dev_host::ProductDevOperationResult::rejected(operation, format!("{error:?}"))
            .map_err(|host_error| self.runtime_error("GENERATED_OPERATION_RESULT", host_error))
    }

    fn lifecycle_result<E: Debug>(
        &self,
        operation: product_dev_host::ProductDevOperationKind,
        transition: Result<(), E>,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let result = match transition {
            Ok(()) => self.accepted_operation(operation)?,
            Err(error) => self.rejected_operation(operation, error)?,
        };
        self.receipt(result, &[])
    }

    fn bundle() -> Result<product_dev_host::ProductDevBundle, String> {
        product_dev_host::ProductDevBundle::new(vec![__BUNDLE_ENTRIES__])
            .map_err(|error| error.to_string())
    }
}

impl product_dev_host::ProductDevRuntime for GeneratedProductDevRuntime {
    fn lifecycle(
        &mut self,
        operation: product_dev_host::ProductDevLifecycleOperation,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        match operation {
            product_dev_host::ProductDevLifecycleOperation::Start => {
                let transition = self.composition.start().map(|_| ());
                self.lifecycle_result(product_dev_host::ProductDevOperationKind::Start, transition)
            }
            product_dev_host::ProductDevLifecycleOperation::Pause => {
                let transition = self.composition.pause().map(|_| ());
                self.lifecycle_result(product_dev_host::ProductDevOperationKind::Pause, transition)
            }
            product_dev_host::ProductDevLifecycleOperation::Resume => {
                let transition = self.composition.resume().map(|_| ());
                self.lifecycle_result(product_dev_host::ProductDevOperationKind::Resume, transition)
            }
            product_dev_host::ProductDevLifecycleOperation::Restart => {
                let transition = self.composition.restart().map(|_| ());
                self.lifecycle_result(product_dev_host::ProductDevOperationKind::Restart, transition)
            }
            product_dev_host::ProductDevLifecycleOperation::Shutdown => {
                let transition = self.composition.shutdown().map(|_| ());
                self.lifecycle_result(product_dev_host::ProductDevOperationKind::Shutdown, transition)
            }
            product_dev_host::ProductDevLifecycleOperation::ReportFault => {
                let transition = self.composition.report_fault().map(|_| ());
                self.lifecycle_result(
                    product_dev_host::ProductDevOperationKind::ReportFault,
                    transition,
                )
            }
        }
    }

    fn input(
        &mut self,
        batch: product_dev_host::ProductDevInputBatch,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevInputResult>, product_dev_host::ProductDevRuntimeError> {
        let count = batch.events().len();
        let result = batch.events().iter().cloned().try_for_each(|event| self.composition.ingest(event));
        let result = match result {
            Ok(()) => product_dev_host::ProductDevInputResult::accepted(count, self.binding(), self.readout()),
            Err(error) => product_dev_host::ProductDevInputResult::rejected(format!("{error:?}")),
        }
        .map_err(|error| self.runtime_error("GENERATED_INPUT_RESULT", error))?;
        self.receipt(result, &[])
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: product_dev_host::CanonicalU64,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let result = self.composition.advance_realtime(runtime_lifecycle::HostMonotonicTime::from_nanoseconds(observed_time_ns.get()));
        let steps = match result {
            Ok(steps) => steps,
            Err(error) => {
                let result = self.rejected_operation(product_dev_host::ProductDevOperationKind::AdvanceRealtime, error)?;
                return self.receipt(result, &[]);
            }
        };
        let result = self.accepted_operation(product_dev_host::ProductDevOperationKind::AdvanceRealtime)?;
        self.receipt(result, &steps)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let result = self.composition.demand_step();
        let step = match result {
            Ok(step) => step,
            Err(error) => {
                let result = self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitDemandStep, error)?;
                return self.receipt(result, &[]);
            }
        };
        let result = self.accepted_operation(product_dev_host::ProductDevOperationKind::AdmitDemandStep)?;
        self.receipt(result, std::slice::from_ref(&step))
    }

    fn admit_external_step(
        &mut self,
        step: product_dev_host::CanonicalU64,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevOperationResult>, product_dev_host::ProductDevRuntimeError> {
        let result = self.composition.external_step(runtime_lifecycle::ExternalStep::new(step.get()));
        let step = match result {
            Ok(step) => step,
            Err(error) => {
                let result = self.rejected_operation(product_dev_host::ProductDevOperationKind::AdmitExternalStep, error)?;
                return self.receipt(result, &[]);
            }
        };
        let result = self.accepted_operation(product_dev_host::ProductDevOperationKind::AdmitExternalStep)?;
        self.receipt(result, std::slice::from_ref(&step))
    }

    fn complete_timeline(
        &mut self,
        completion: product_dev_host::ProductDevTimelineCompletion,
    ) -> Result<product_dev_host::ProductDevRuntimeReceipt<product_dev_host::ProductDevTimelineCompletionResult>, product_dev_host::ProductDevRuntimeError> {
        let ticket = completion.envelope().ticket().value();
        let result = self.composition.admit_timeline_completion(completion.into_envelope());
        let result = match result {
            Ok(_) => product_dev_host::ProductDevTimelineCompletionResult::accepted(
                product_dev_host::CanonicalU64::new(ticket),
                self.binding(),
                self.readout(),
            ),
            Err(error) => product_dev_host::ProductDevTimelineCompletionResult::rejected(
                product_dev_host::CanonicalU64::new(ticket),
                format!("{error:?}"),
            ),
        }
        .map_err(|error| self.runtime_error("GENERATED_TIMELINE_RESULT", error))?;
        self.receipt(result, &[])
    }
}

pub fn product_bundle() -> Result<product_dev_host::ProductDevBundle, String> {
    GeneratedProductDevRuntime::bundle()
}

pub fn start_host(port: u16) -> Result<product_dev_host::RunningProductDevHost, String> {
    let runtime = GeneratedProductDevRuntime::new()?;
    let bundle = product_bundle()?;
    product_dev_host::ProductDevHost::start(
        runtime,
        product_dev_host::ProductDevHostConfig::new(port, bundle),
    )
    .map_err(|error| error.to_string())
}

pub fn run(port: u16) {
    let host = start_host(port).expect("generated Product Dev Host starts");
    println!("{}", host.origin());
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    host.shutdown().expect("generated Product Dev Host shuts down");
}
"#
    .replace("__ADAPTER_TYPE__", &adapter_type)
    .replace("__KERNEL_DESCRIPTORS__", kernel_descriptors)
    .replace("__KERNEL_IMPORT__", kernel_import)
    .replace(
        "__RUNTIME_RESOURCE_DECLARATION__",
        &runtime_resource_declaration.replace("__RUNTIME_RESOURCES__", runtime_resources),
    )
    .replace("__KERNEL_VALIDATION__", &kernel_validation)
    .replace("__ADAPTER_CONSTRUCTION__", &adapter_construction)
    .replace("__MUTATION_CATALOG__", &mutation_catalog)
    .replace("__STANDARD_CAPABILITIES__", standard_capabilities)
    .replace("__BUNDLE_ENTRIES__", bundle_entries);
    let start_marker = "__EMPTY_ADAPTER_START__";
    let end_marker = "__EMPTY_ADAPTER_END__";
    if has_kernel {
        let start = source
            .find(start_marker)
            .expect("generated empty adapter start marker");
        let end = source
            .find(end_marker)
            .map(|index| index + end_marker.len())
            .expect("generated empty adapter end marker");
        source.replace_range(start..end, "");
    } else {
        source = source.replace(start_marker, "").replace(end_marker, "");
    }
    source
}

fn browser_content_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()? {
        "html" => Some("text/html; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "json" => Some("application/json; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "wav" => Some("audio/wav"),
        "wasm" => Some("application/wasm"),
        "rmesh" => Some("application/octet-stream"),
        _ => None,
    }
}

fn render_kind(kind: CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::System => "System",
        CapabilityKind::Operation => "Operation",
        CapabilityKind::Query => "Query",
        CapabilityKind::Projection => "Projection",
        CapabilityKind::Migration => "Migration",
    }
}

fn render_uses(uses: product_model::CapabilityUses) -> String {
    let mut values = Vec::new();
    for (usage, name) in [
        (CapabilityUse::InputMap, "INPUT_MAP"),
        (CapabilityUse::Schedule, "SCHEDULE"),
        (CapabilityUse::Timeline, "TIMELINE"),
    ] {
        if uses.contains(usage) {
            values.push(format!("product_model::CapabilityUses::{name}"));
        }
    }
    if values.is_empty() {
        "product_model::CapabilityUses::NONE".to_owned()
    } else {
        let first = values[0].clone();
        values
            .into_iter()
            .skip(1)
            .fold(first, |left, right| format!("{left}.union({right})"))
    }
}

fn render_availability(availability: CapabilityAvailability) -> String {
    match availability {
        CapabilityAvailability::Linkable => {
            "product_model::CapabilityAvailability::Linkable".to_owned()
        }
        CapabilityAvailability::Unavailable { reason } => format!(
            "product_model::CapabilityAvailability::Unavailable {{ reason: {} }}",
            rust_string(reason)
        ),
    }
}

fn render_str_slice(values: &[&str]) -> String {
    format!(
        "&[{}]",
        values
            .iter()
            .map(|value| rust_string(value))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn validate_browser_path(path: &str) -> Result<(), ProductAssemblyError> {
    let forbidden_name = path.rsplit('/').next().is_some_and(|name| {
        name.starts_with(".env")
            || name == "package.json"
            || name == "package-lock.json"
            || name == "pnpm-lock.yaml"
            || name == "yarn.lock"
            || name.starts_with("tsconfig")
            || name.starts_with("vite.config")
            || name.starts_with("webpack.config")
            || name.starts_with("rollup.config")
            || name.starts_with("esbuild.config")
    });
    if path.split('/').any(|component| component == "node_modules")
        || path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".map")
        || forbidden_name
    {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_FORBIDDEN_FILE",
            path,
            "browser closure cannot contain TypeScript, source maps, node_modules, or tool configuration",
        ));
    }
    let fixed = matches!(
        path,
        BROWSER_INDEX_PATH
            | BROWSER_MAIN_PATH
            | BROWSER_BRIDGE_PATH
            | BROWSER_ENGINE_PATH
            | BROWSER_RUNTIME_ADAPTER_PATH
            | BROWSER_RENDERER_PRELOAD_PATH
    );
    if !fixed && !path.starts_with("ui/") && !path.starts_with("assets/") {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_PATH",
            path,
            "additional browser files must remain in ui/ or assets/",
        ));
    }
    Ok(())
}

fn validate_browser_bytes(path: &str, bytes: &[u8]) -> Result<(), ProductAssemblyError> {
    if bytes.is_empty() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_EMPTY_FILE",
            path,
            "browser closure files must contain materialized bytes",
        ));
    }
    let textual = matches!(
        path,
        BROWSER_INDEX_PATH
            | BROWSER_MAIN_PATH
            | BROWSER_BRIDGE_PATH
            | BROWSER_ENGINE_PATH
            | BROWSER_RUNTIME_ADAPTER_PATH
            | BROWSER_RENDERER_PRELOAD_PATH
    ) || path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".css")
        || path.ends_with(".html")
        || path.ends_with(".json")
        || path.ends_with(".svg");
    if !textual {
        return Ok(());
    }
    let text = std::str::from_utf8(bytes).map_err(|error| {
        ProductAssemblyError::new("ASSEMBLY_BROWSER_UTF8", path, error.to_string())
    })?;
    const ABSOLUTE_IMPORT_MARKERS: &[&str] = &[
        "from '/",
        "from \"/",
        "import('/",
        "import(\"/",
        "src='/",
        "src=\"/",
        "href='/",
        "href=\"/",
        "url('/",
        "url(\"/",
        "fetch('/",
        "fetch(\"/",
        "new URL('/",
        "new URL(\"/",
    ];
    if ABSOLUTE_IMPORT_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
    {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_ABSOLUTE_IMPORT",
            path,
            "browser closure imports and resource references must remain product-relative",
        ));
    }
    Ok(())
}

fn validate_dependency_path(path: &str, code: &'static str) -> Result<(), ProductAssemblyError> {
    if path.is_empty()
        || path.len() > 512
        || path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_whitespace)
        || path.bytes().any(|byte| byte.is_ascii_control())
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == ".")
    {
        return Err(ProductAssemblyError::new(
            code,
            "Cargo.toml",
            "Cargo dependency path must be bounded, relative, slash-separated, and free of empty or dot segments",
        ));
    }
    Ok(())
}

fn extract_cargo_dependency_path(
    cargo: &[u8],
    dependency: &str,
) -> Result<Option<String>, ProductAssemblyError> {
    let text = std::str::from_utf8(cargo).map_err(|error| {
        ProductAssemblyError::new("ASSEMBLY_CARGO_UTF8", "Cargo.toml", error.to_string())
    })?;
    let prefix = format!("{dependency} = {{ path = \"");
    let mut result = None;
    for line in text.lines().map(str::trim) {
        let Some(rest) = line.strip_prefix(&prefix) else {
            continue;
        };
        let Some((path, suffix)) = rest.split_once('"') else {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CARGO_DEPENDENCY",
                "Cargo.toml",
                format!("{dependency} dependency path is not a closed quoted string"),
            ));
        };
        if suffix.trim() != "}" {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CARGO_DEPENDENCY",
                "Cargo.toml",
                format!("{dependency} dependency declaration contains unexpected fields"),
            ));
        }
        if result.replace(path.to_owned()).is_some() {
            return Err(ProductAssemblyError::new(
                "ASSEMBLY_CARGO_DEPENDENCY",
                "Cargo.toml",
                format!("{dependency} dependency is declared more than once"),
            ));
        }
    }
    Ok(result)
}

fn require_browser_reference(
    files: &[PublicationFile],
    source_path: &str,
    required_reference: &str,
) -> Result<(), ProductAssemblyError> {
    let source = files
        .iter()
        .find(|file| file.relative_path().as_str() == source_path)
        .expect("required browser file was checked before references");
    let text = std::str::from_utf8(source.bytes()).map_err(|error| {
        ProductAssemblyError::new("ASSEMBLY_BROWSER_UTF8", source_path, error.to_string())
    })?;
    if !text.contains(required_reference) {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_BROWSER_REFERENCE",
            source_path,
            format!("browser composition root must reference `{required_reference}`"),
        ));
    }
    Ok(())
}

fn insert_file(
    files: &mut BTreeMap<String, ProductFile>,
    file: ProductFile,
) -> Result<(), ProductAssemblyError> {
    let key = file.relative_path.as_str().to_owned();
    if files.insert(key.clone(), file).is_some() {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_DUPLICATE_SOURCE",
            key,
            "one source path was admitted more than once",
        ));
    }
    Ok(())
}

fn validate_generated_count(files: &[PublicationFile]) -> Result<(), ProductAssemblyError> {
    if files.len() > MAX_GENERATED_FILES {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_GENERATED_FILE_COUNT",
            "product-assembly",
            format!("generated output is limited to {MAX_GENERATED_FILES} files"),
        ));
    }
    Ok(())
}

fn validate_total_receipt_bytes(
    entries: &[AssemblyClosureEntry],
) -> Result<(), ProductAssemblyError> {
    let total = entries.iter().try_fold(0usize, |total, entry| {
        total.checked_add(entry.byte_length()).ok_or_else(|| {
            ProductAssemblyError::new(
                "ASSEMBLY_TOTAL_BYTES_BOUNDS",
                "assembly.json",
                "receipt byte accounting overflowed",
            )
        })
    })?;
    if total > crate::MAX_ASSEMBLY_TOTAL_BYTES {
        return Err(ProductAssemblyError::new(
            "ASSEMBLY_TOTAL_BYTES_BOUNDS",
            "assembly.json",
            format!(
                "all closure files are limited to {} bytes",
                crate::MAX_ASSEMBLY_TOTAL_BYTES
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod kernel_package_tests {
    use super::*;

    fn file(path: &str, bytes: &str) -> ProductFile {
        ProductFile {
            relative_path: ProductPath::parse(path.to_owned()).expect("test path"),
            bytes: bytes.as_bytes().to_vec(),
        }
    }

    #[test]
    fn package_mode_rewrites_the_fixed_engine_and_closes_local_crates() {
        let files = BTreeMap::from([
            (
                "kernel/Cargo.toml".to_owned(),
                file(
                    "kernel/Cargo.toml",
                    "[package]\nname = \"example-kernel\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nrusty-engine = { path = \"../../wrong\" }\nhelper = { path = \"support/helper\" }\n",
                ),
            ),
            (
                "kernel/support/helper/Cargo.toml".to_owned(),
                file(
                    "kernel/support/helper/Cargo.toml",
                    "[package]\nname = \"example-helper\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nrusty-engine = { path = \"../../../../wrong\" }\n",
                ),
            ),
        ]);
        let package = admit_kernel_package(
            &ProductPath::parse("kernel/Cargo.toml".to_owned()).expect("package path"),
            &files,
            "../rusty-engine",
        )
        .expect("admitted package");
        assert_eq!(package.package_name, "example-kernel");
        let root = std::str::from_utf8(
            package
                .cargo_rewrites
                .get(&ProductPath::parse("kernel/Cargo.toml".to_owned()).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(root.contains("path = \"../../rusty-engine\""));
        assert_eq!(package.cargo_rewrites.len(), 2);
    }

    #[test]
    fn package_mode_strips_only_the_admitted_empty_root_workspace_when_copying() {
        let files = BTreeMap::from([(
            "kernel/Cargo.toml".to_owned(),
            file(
                "kernel/Cargo.toml",
                "[package]\nname = \"example-kernel\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\nrusty-engine = { path = \"../../wrong\" }\n",
            ),
        )]);
        let package = admit_kernel_package(
            &ProductPath::parse("kernel/Cargo.toml".to_owned()).unwrap(),
            &files,
            "../rusty-engine",
        )
        .expect("empty workspace is an authored opt-out");
        let copied = std::str::from_utf8(
            package
                .cargo_rewrites
                .get(&ProductPath::parse("kernel/Cargo.toml".to_owned()).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(!copied.contains("[workspace]"));
    }

    #[test]
    fn package_mode_rejects_local_dependency_escape_and_registry_dependency() {
        for (dependency, expected) in [
            (
                "helper = { path = \"../../outside\" }",
                "ASSEMBLY_KERNEL_PACKAGE_ESCAPE",
            ),
            (
                "helper = \"1\"",
                "ASSEMBLY_KERNEL_PACKAGE_REGISTRY_DEPENDENCY",
            ),
        ] {
            let files = BTreeMap::from([(
                "kernel/Cargo.toml".to_owned(),
                file(
                    "kernel/Cargo.toml",
                    &format!(
                        "[package]\nname = \"example-kernel\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nrusty-engine = {{ path = \"../../engine\" }}\n{dependency}\n"
                    ),
                ),
            )]);
            let error = admit_kernel_package(
                &ProductPath::parse("kernel/Cargo.toml".to_owned()).unwrap(),
                &files,
                "../rusty-engine",
            )
            .expect_err("unsafe dependency rejected");
            assert_eq!(error.diagnostic().code(), expected);
        }
    }

    #[test]
    fn package_mode_rejects_all_workspace_inheritance_forms() {
        for manifest in [
            "[package]\nname = \"example-kernel\"\nversion = \"0.1.0\"\nedition = \"2021\"\nworkspace = \"outer\"\n\n[dependencies]\nrusty-engine = { path = \"../../engine\" }\n",
            "[package]\nname = \"example-kernel\"\nversion.workspace = true\nedition = \"2021\"\n\n[dependencies]\nrusty-engine = { path = \"../../engine\" }\n",
            "[package]\nname = \"example-kernel\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lints]\nworkspace = true\n\n[dependencies]\nrusty-engine = { path = \"../../engine\" }\n",
        ] {
            let files = BTreeMap::from([(
                "kernel/Cargo.toml".to_owned(),
                file(
                    "kernel/Cargo.toml",
                    manifest,
                ),
            )]);
            let error = admit_kernel_package(
                &ProductPath::parse("kernel/Cargo.toml".to_owned()).unwrap(),
                &files,
                "../rusty-engine",
            )
            .expect_err("ambient workspace inheritance rejected");
            assert_eq!(
                error.diagnostic().code(),
                "ASSEMBLY_KERNEL_PACKAGE_WORKSPACE"
            );
        }
        let dependency: toml::Value =
            toml::from_str("[dependencies]\nhelper = { workspace = true }\n").unwrap();
        assert!(contains_workspace_inheritance(&dependency));
    }

    #[test]
    fn package_mode_rejects_build_residue_instead_of_hashing_it_as_source() {
        for path in [
            "kernel/Cargo.lock",
            "kernel/target/debug/kernel",
            "kernel/.cargo/config.toml",
        ] {
            let files = BTreeMap::from([(
                path.to_owned(),
                file(path, "generated residue must not be admitted"),
            )]);
            let error = validate_kernel_package_source_closure(&files)
                .expect_err("kernel build residue rejected");
            assert_eq!(
                error.diagnostic().code(),
                "ASSEMBLY_KERNEL_PACKAGE_NON_SOURCE"
            );
            assert_eq!(error.diagnostic().path(), path);
        }
    }
}
