//! V1 staged Product admission for the packaged Rusty host, from a loose
//! directory or a container (`product_container::ProductSource`).
//!
//! This is deployment metadata, not a product callback protocol.  It selects
//! exact staged files before the trusted product is loaded and makes the
//! browser host's product UI bytes immutable for the lifetime of the launch.

use std::{
    fs,
    net::Ipv4Addr,
    path::{Component, Path, PathBuf},
};

use csharp_product_runtime::ProductSource;
use product_container::{is_relative_path, join};

use csharp_engine_abi::NativeInputCursorMode;
use product_host::{
    ProductHostBootstrapInput, ProductHostBootstrapLifecycle, ProductHostBootstrapProduct,
    ProductHostBootstrapRenderer, ProductHostBootstrapUi, ProductHostBootstrapUiProjection,
    ProductHostBrowserBootstrap, ProductHostBundleEntry, ProductHostCursorMode,
    ProductHostRuntimeMode, PRODUCT_HOST_BOOTSTRAP_PATH,
};
use runtime_input::{CompiledInputMappings, DirectInputIntentDescriptor, RuntimeInputMapping};
use runtime_lifecycle::{
    validate_runtime_identity, RealtimeLifecycleConfig, RuntimeLifecycleConfig,
};
use serde::Deserialize;

use super::{content_type, parse_direct_intent, parse_physical_mapping, ProductLoader};

pub(super) const PRODUCT_MANIFEST_NAME: &str = "product.json";
const PRODUCT_ARTIFACT: &str = "rusty.product.bundle";
const PRODUCT_UI_PREFIX: &str = "product-ui";

#[derive(Debug)]
pub(super) struct ProductBundle {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) native_module: Option<PathBuf>,
    pub(super) coreclr_assembly: Option<PathBuf>,
    pub(super) coreclr_runtimeconfig: Option<PathBuf>,
    pub(super) source: ProductSource,
    /// The content root within `source`.
    pub(super) content_root: String,
    /// The UI root within `source`.
    ui_root: String,
    ui_entry: String,
    ui_projection: Option<ProductUiProjection>,
    renderer_lighting: ProductRendererLighting,
    /// Where the runtime draws (`renderer.output`).
    pub(super) render_output: csharp_product_runtime::RenderOutput,
    /// A missing audio device fails the load (`audio.output`).
    pub(super) audio_device_required: bool,
    pub(super) lifecycle: RuntimeLifecycleConfig,
    pub(super) lifecycle_mode: ProductHostRuntimeMode,
    pub(super) direct_intents: Vec<DirectInputIntentDescriptor>,
    pub(super) physical_mappings: Vec<RuntimeInputMapping>,
    pub(super) input_cursor_mode: ProductInputCursorMode,
    pub(super) bind_host: Ipv4Addr,
    pub(super) port: u16,
    pub(super) live_debug: bool,
}

impl ProductBundle {
    pub(super) fn read(source: &ProductSource) -> Result<Self, String> {
        let root = source.native_root();
        let bytes = source
            .read(PRODUCT_MANIFEST_NAME)
            .map_err(|error| field_error("manifest", error))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|error| field_error("manifest", format!("invalid JSON: {error}")))?;
        if manifest.artifact != PRODUCT_ARTIFACT {
            return Err(field_error(
                "artifact",
                format!("must be `{PRODUCT_ARTIFACT}`"),
            ));
        }
        if manifest.schema_version != 1 {
            return Err(field_error("schemaVersion", "must be 1"));
        }
        validate_runtime_identity(&manifest.product.id).map_err(|_| {
            field_error("product.id", "must be a bounded lowercase runtime identity")
        })?;
        if manifest.product.title.trim().is_empty() || manifest.product.title.len() > 160 {
            return Err(field_error(
                "product.title",
                "must contain 1 to 160 characters",
            ));
        }

        let native_module = manifest
            .native_aot
            .map(|native| resolve_regular_file(root, &native.module, "nativeAot.module"))
            .transpose()?;
        let (coreclr_assembly, coreclr_runtimeconfig) = match manifest.coreclr {
            Some(coreclr) => (
                Some(resolve_regular_file(
                    root,
                    &coreclr.assembly,
                    "coreclr.assembly",
                )?),
                Some(resolve_regular_file(
                    root,
                    &coreclr.runtimeconfig,
                    "coreclr.runtimeconfig",
                )?),
            ),
            None => (None, None),
        };
        if native_module.is_none() && coreclr_assembly.is_none() {
            return Err(field_error(
                "nativeAot/coreclr",
                "must declare at least one product artifact",
            ));
        }

        let ui_root = product_directory(source, "", &manifest.ui.root, "ui.root")?;
        let ui_entry = product_file(source, &ui_root, &manifest.ui.entry, "ui.entry")?;
        // Validate the declared assets path even though the complete UI root is
        // staged.  Modules can import adjacent assets without a second UI file
        // vocabulary, while this retains an explicit assets declaration.
        product_directory(source, &ui_root, &manifest.ui.assets, "ui.assets")?;
        let content_root = product_directory(source, "", &manifest.content.root, "content.root")?;
        let ui_projection = manifest
            .ui_projection
            .map(ProductUiProjection::from_manifest)
            .transpose()?;
        let render_output = match manifest.renderer.output.as_deref() {
            None | Some("stream") => csharp_product_runtime::RenderOutput::Stream,
            Some("window") => csharp_product_runtime::RenderOutput::Window,
            Some(_) => return Err(field_error("renderer.output", "must be stream or window")),
        };
        let audio_device_required = match manifest.audio.output.as_deref() {
            None | Some("device-optional") => false,
            Some("device-required") => true,
            Some(_) => {
                return Err(field_error(
                    "audio.output",
                    "must be device-optional or device-required",
                ))
            }
        };
        let renderer_lighting = ProductRendererLighting::from_manifest(manifest.renderer)?;

        let (lifecycle, lifecycle_mode) = lifecycle(&manifest.lifecycle)?;
        let (direct_intents, physical_mappings, input_cursor_mode) = input(&manifest.input)?;
        let bind_host = manifest
            .server
            .bind_host
            .parse::<Ipv4Addr>()
            .map_err(|_| field_error("server.bindHost", "must be an IPv4 address"))?;

        Ok(Self {
            id: manifest.product.id,
            title: manifest.product.title,
            native_module,
            coreclr_assembly,
            coreclr_runtimeconfig,
            source: source.clone(),
            content_root,
            ui_root,
            ui_entry,
            ui_projection,
            renderer_lighting,
            render_output,
            audio_device_required,
            lifecycle,
            lifecycle_mode,
            direct_intents,
            physical_mappings,
            input_cursor_mode,
            bind_host,
            port: manifest.server.port,
            live_debug: manifest.server.live_debug,
        })
    }

    pub(super) fn selected_artifacts(
        &self,
        loader: ProductLoader,
    ) -> Result<(&Path, Option<&Path>), String> {
        match loader {
            ProductLoader::NativeAot => self
                .native_module
                .as_deref()
                .map(|module| (module, None))
                .ok_or_else(|| {
                    field_error(
                        "nativeAot.module",
                        "is required when --loader nativeaot is selected",
                    )
                }),
            ProductLoader::CoreClr => match (&self.coreclr_assembly, &self.coreclr_runtimeconfig) {
                (Some(assembly), Some(runtimeconfig)) => Ok((assembly, Some(runtimeconfig))),
                _ => Err(field_error(
                    "coreclr",
                    "assembly and runtimeconfig are required when --loader coreclr is selected",
                )),
            },
        }
    }

    /// The manifest's world and viewmodel default light rigs, on or off.
    pub(super) fn default_lights(&self) -> (bool, bool) {
        self.renderer_lighting.enabled()
    }

    pub(super) fn shadows_enabled(&self) -> bool {
        self.renderer_lighting.shadows
    }

    pub(super) fn browser_entries(&self) -> Result<Vec<ProductHostBundleEntry>, String> {
        let mut entries = Vec::new();
        self.collect_ui(&mut entries)?;
        let bootstrap = ProductHostBrowserBootstrap {
            product: ProductHostBootstrapProduct {
                id: self.id.clone(),
                title: self.title.clone(),
            },
            ui: ProductHostBootstrapUi {
                entry: format!("{PRODUCT_UI_PREFIX}/{}", self.ui_entry),
            },
            lifecycle: ProductHostBootstrapLifecycle {
                mode: self.lifecycle_mode,
            },
            input: ProductHostBootstrapInput {
                cursor_mode: self.input_cursor_mode.bootstrap(),
            },
            ui_projection: self.ui_projection.as_ref().map(|projection| {
                ProductHostBootstrapUiProjection {
                    expected_stream: projection.expected_stream.clone(),
                    expected_contract: projection.expected_contract.clone(),
                }
            }),
            renderer: ProductHostBootstrapRenderer {
                output: self.render_output,
            },
        };
        entries.push(
            ProductHostBundleEntry::new(
                PRODUCT_HOST_BOOTSTRAP_PATH,
                "application/json; charset=utf-8",
                serde_json::to_vec(&bootstrap).expect("fixed Product browser bootstrap encodes"),
            )
            .map_err(|error| error.to_string())?,
        );
        Ok(entries)
    }
}

fn lifecycle(
    value: &ManifestLifecycle,
) -> Result<(RuntimeLifecycleConfig, ProductHostRuntimeMode), String> {
    match value.mode.as_str() {
        "realtime" => {
            let fixed_step = value.fixed_step.as_ref().ok_or_else(|| {
                field_error("lifecycle.fixedStep", "is required for realtime mode")
            })?;
            let config = RealtimeLifecycleConfig::new(fixed_step.hz, fixed_step.max_catch_up_steps)
                .map_err(|error| field_error("lifecycle.fixedStep", error.to_string()))?;
            Ok((
                RuntimeLifecycleConfig::Realtime(config),
                ProductHostRuntimeMode::Realtime,
            ))
        }
        "demand" => {
            if value.fixed_step.is_some() {
                return Err(field_error(
                    "lifecycle.fixedStep",
                    "is valid only for realtime mode",
                ));
            }
            Ok((
                RuntimeLifecycleConfig::Demand,
                ProductHostRuntimeMode::Demand,
            ))
        }
        "external" => {
            if value.fixed_step.is_some() {
                return Err(field_error(
                    "lifecycle.fixedStep",
                    "is valid only for realtime mode",
                ));
            }
            Ok((
                RuntimeLifecycleConfig::External,
                ProductHostRuntimeMode::External,
            ))
        }
        _ => Err(field_error(
            "lifecycle.mode",
            "must be realtime, demand, or external",
        )),
    }
}

fn input(
    value: &ManifestInput,
) -> Result<
    (
        Vec<DirectInputIntentDescriptor>,
        Vec<RuntimeInputMapping>,
        ProductInputCursorMode,
    ),
    String,
> {
    let direct_intents = value
        .intents
        .iter()
        .map(|intent| {
            parse_direct_intent(&format!("{}={}", intent.id, intent.value))
                .map_err(|error| field_error("input.intents", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let physical_mappings = value
        .mappings
        .iter()
        .map(|mapping| {
            parse_physical_mapping(&format!(
                "{}={}:{}",
                mapping.id, mapping.intent, mapping.trigger
            ))
            .map_err(|error| field_error("input.mappings", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    CompiledInputMappings::standard(direct_intents.clone(), physical_mappings.clone())
        .map_err(|error| field_error("input", error.to_string()))?;
    Ok((
        direct_intents,
        physical_mappings,
        ProductInputCursorMode::parse(value.cursor_mode.as_deref())?,
    ))
}

fn relative_path(value: &str, field: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(field_error(
            field,
            "must be a non-empty relative non-escaping path",
        ));
    }
    Ok(path.to_owned())
}

/// A native artifact: a regular file on disk under `root`, loaded by path.
fn resolve_regular_file(root: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    let path = root.join(relative_path(value, field)?);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        field_error(
            field,
            format!("could not read `{}`: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(field_error(field, "must be a regular file, not a symlink"));
    }
    let canonical =
        fs::canonicalize(path).map_err(|error| field_error(field, error.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(field_error(field, "resolved outside Product root"));
    }
    Ok(canonical)
}

/// `parent/value` in the staged Product, when it names a directory.
fn product_directory(
    source: &ProductSource,
    parent: &str,
    value: &str,
    field: &str,
) -> Result<String, String> {
    let path = product_path(parent, value, field)?;
    if !source.is_dir(&path) {
        return Err(field_error(field, "must be a directory, not a symlink"));
    }
    Ok(path)
}

/// `value` relative to `parent`, when `parent/value` names a regular file.
fn product_file(
    source: &ProductSource,
    parent: &str,
    value: &str,
    field: &str,
) -> Result<String, String> {
    if !source.is_file(&product_path(parent, value, field)?) {
        return Err(field_error(field, "must be a regular file, not a symlink"));
    }
    Ok(value.to_owned())
}

fn product_path(parent: &str, value: &str, field: &str) -> Result<String, String> {
    if !is_relative_path(value) {
        return Err(field_error(
            field,
            "must be a non-empty relative non-escaping path",
        ));
    }
    Ok(join(parent, value))
}

impl ProductBundle {
    fn collect_ui(&self, entries: &mut Vec<ProductHostBundleEntry>) -> Result<(), String> {
        let ui_error = |error| field_error("ui.root", error);
        for path in self.source.files(&self.ui_root).map_err(ui_error)? {
            let relative = &path[self.ui_root.len() + 1..];
            let media_type = content_type(relative).ok_or_else(|| {
                field_error(
                    "ui.root",
                    format!("file `{relative}` has no admitted content type"),
                )
            })?;
            let bytes = self.source.read(&path).map_err(ui_error)?;
            entries.push(
                ProductHostBundleEntry::new(
                    format!("{PRODUCT_UI_PREFIX}/{relative}"),
                    media_type,
                    bytes.into_owned(),
                )
                .map_err(|error| field_error("ui.root", error.to_string()))?,
            );
        }
        Ok(())
    }
}

fn field_error(field: &str, detail: impl std::fmt::Display) -> String {
    format!("{PRODUCT_MANIFEST_NAME}:{field}: {detail}")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    artifact: String,
    schema_version: u32,
    product: ManifestProduct,
    native_aot: Option<ManifestNativeAot>,
    coreclr: Option<ManifestCoreClr>,
    ui: ManifestUi,
    content: ManifestContent,
    ui_projection: Option<ManifestUiProjection>,
    #[serde(default)]
    renderer: ManifestRenderer,
    #[serde(default)]
    audio: ManifestAudio,
    lifecycle: ManifestLifecycle,
    input: ManifestInput,
    #[serde(default)]
    server: ManifestServer,
}

#[derive(Debug, Deserialize)]
struct ManifestProduct {
    id: String,
    title: String,
}
#[derive(Debug, Deserialize)]
struct ManifestNativeAot {
    module: String,
}
#[derive(Debug, Deserialize)]
struct ManifestCoreClr {
    assembly: String,
    runtimeconfig: String,
}
#[derive(Debug, Deserialize)]
struct ManifestUi {
    root: String,
    entry: String,
    assets: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestUiProjection {
    expected_stream: String,
    expected_contract: String,
}
#[derive(Debug)]
struct ProductUiProjection {
    expected_stream: String,
    expected_contract: String,
}
impl ProductUiProjection {
    fn from_manifest(value: ManifestUiProjection) -> Result<Self, String> {
        validate_runtime_identity(&value.expected_stream).map_err(|_| {
            field_error(
                "uiProjection.expectedStream",
                "must be a bounded lowercase runtime identity",
            )
        })?;
        validate_runtime_identity(&value.expected_contract).map_err(|_| {
            field_error(
                "uiProjection.expectedContract",
                "must be a bounded lowercase runtime identity",
            )
        })?;
        Ok(Self {
            expected_stream: value.expected_stream,
            expected_contract: value.expected_contract,
        })
    }
}
#[derive(Debug, Deserialize)]
struct ManifestContent {
    root: String,
}
#[derive(Debug, Default, Deserialize)]
struct ManifestRenderer {
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    lighting: ManifestRendererLighting,
}
#[derive(Debug, Default, Deserialize)]
struct ManifestAudio {
    #[serde(default)]
    output: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRendererLighting {
    #[serde(default)]
    shadows: Option<String>,
    #[serde(default)]
    default_lights: ManifestDefaultLights,
}
#[derive(Debug, Deserialize)]
struct ManifestDefaultLights {
    #[serde(default = "neutral_lights")]
    world: String,
    #[serde(default = "neutral_lights")]
    viewmodel: String,
}
impl Default for ManifestDefaultLights {
    fn default() -> Self {
        Self {
            world: neutral_lights(),
            viewmodel: neutral_lights(),
        }
    }
}
fn neutral_lights() -> String {
    "neutral".to_owned()
}
#[derive(Debug, Clone, Copy)]
enum ProductDefaultLights {
    Neutral,
    Disabled,
}
impl ProductDefaultLights {
    fn parse(value: String, field: &str) -> Result<Self, String> {
        match value.as_str() {
            "neutral" => Ok(Self::Neutral),
            "disabled" => Ok(Self::Disabled),
            _ => Err(field_error(field, "must be neutral or disabled")),
        }
    }
}
#[derive(Debug)]
struct ProductRendererLighting {
    shadows: bool,
    world: ProductDefaultLights,
    viewmodel: ProductDefaultLights,
}
impl ProductRendererLighting {
    /// Whether the world and viewmodel default rigs are on.
    fn enabled(&self) -> (bool, bool) {
        (
            matches!(self.world, ProductDefaultLights::Neutral),
            matches!(self.viewmodel, ProductDefaultLights::Neutral),
        )
    }

    fn from_manifest(value: ManifestRenderer) -> Result<Self, String> {
        let shadows = match value.lighting.shadows.as_deref() {
            None | Some("disabled") => false,
            Some("enabled") => true,
            Some(_) => {
                return Err(field_error(
                    "renderer.lighting.shadows",
                    "must be enabled or disabled",
                ))
            }
        };
        Ok(Self {
            shadows,
            world: ProductDefaultLights::parse(
                value.lighting.default_lights.world,
                "renderer.lighting.defaultLights.world",
            )?,
            viewmodel: ProductDefaultLights::parse(
                value.lighting.default_lights.viewmodel,
                "renderer.lighting.defaultLights.viewmodel",
            )?,
        })
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestLifecycle {
    mode: String,
    fixed_step: Option<ManifestFixedStep>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFixedStep {
    hz: u32,
    max_catch_up_steps: u32,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestInput {
    #[serde(default)]
    cursor_mode: Option<String>,
    #[serde(default)]
    intents: Vec<ManifestIntent>,
    #[serde(default)]
    mappings: Vec<ManifestMapping>,
}
#[derive(Debug, Default, Clone, Copy)]
pub(super) enum ProductInputCursorMode {
    #[default]
    PointerLock,
    Unlocked,
}
impl ProductInputCursorMode {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("pointer-lock") => Ok(Self::PointerLock),
            Some("unlocked") => Ok(Self::Unlocked),
            Some(_) => Err(field_error(
                "input.cursorMode",
                "must be pointer-lock or unlocked",
            )),
        }
    }

    pub(super) fn native(self) -> NativeInputCursorMode {
        match self {
            Self::PointerLock => NativeInputCursorMode::PointerLock,
            Self::Unlocked => NativeInputCursorMode::Unlocked,
        }
    }
    fn bootstrap(self) -> ProductHostCursorMode {
        match self {
            Self::PointerLock => ProductHostCursorMode::PointerLock,
            Self::Unlocked => ProductHostCursorMode::Unlocked,
        }
    }
}
#[derive(Debug, Deserialize)]
struct ManifestIntent {
    id: String,
    value: String,
}
#[derive(Debug, Deserialize)]
struct ManifestMapping {
    id: String,
    intent: String,
    trigger: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestServer {
    #[serde(default = "loopback")]
    bind_host: String,
    #[serde(default)]
    port: u16,
    #[serde(default)]
    live_debug: bool,
}
impl Default for ManifestServer {
    fn default() -> Self {
        Self {
            bind_host: loopback(),
            port: 0,
            live_debug: false,
        }
    }
}
fn loopback() -> String {
    "127.0.0.1".to_owned()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn fixture_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "rusty-product-bundle-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("native")).unwrap();
        fs::create_dir_all(root.join("coreclr")).unwrap();
        fs::create_dir_all(root.join("ui/assets")).unwrap();
        fs::create_dir_all(root.join("content")).unwrap();
        fs::write(root.join("native/product.so"), b"fixture").unwrap();
        fs::write(root.join("coreclr/product.dll"), b"fixture").unwrap();
        fs::write(root.join("coreclr/product.runtimeconfig.json"), b"{}").unwrap();
        fs::write(
            root.join("ui/main.js"),
            b"export function mountProductUi() {} ",
        )
        .unwrap();
        fs::write(root.join("ui/assets/marker.json"), b"{}").unwrap();
        fs::write(root.join("content/trial.txt"), b"admitted content").unwrap();
        root
    }

    fn read(root: &Path) -> Result<ProductBundle, String> {
        ProductBundle::read(&ProductSource::open(root).map_err(|error| error.to_string())?)
    }

    fn write_manifest(root: &Path, native_path: &str) {
        fs::write(root.join(PRODUCT_MANIFEST_NAME), format!(r#"{{
          "artifact":"rusty.product.bundle","schemaVersion":1,
          "product":{{"id":"fixture.product","title":"Fixture"}},
          "nativeAot":{{"module":"{native_path}"}},
          "coreclr":{{"assembly":"coreclr/product.dll","runtimeconfig":"coreclr/product.runtimeconfig.json"}},
          "ui":{{"root":"ui","entry":"main.js","assets":"assets"}},
          "content":{{"root":"content"}},
          "uiProjection":{{"expectedStream":"fixture.terrain","expectedContract":"fixture.terrain.v1"}},
          "lifecycle":{{"mode":"realtime","fixedStep":{{"hz":60,"maxCatchUpSteps":2}}}},
          "input":{{"intents":[{{"id":"move.forward","value":"digital"}}],"mappings":[{{"id":"move.forward.w","intent":"move.forward","trigger":"key:key-w:held"}}]}},
          "server":{{"bindHost":"127.0.0.1","port":0,"liveDebug":true}},
          "ignoredV1Field":true
        }}"#)).unwrap();
    }

    #[test]
    fn admits_one_product_root_and_stages_only_prefixed_ui_bytes() {
        let root = fixture_root("staging");
        write_manifest(&root, "native/product.so");
        let product = read(&root).expect("V1 Product bundle admits");
        assert_eq!(
            product
                .selected_artifacts(ProductLoader::NativeAot)
                .unwrap()
                .0,
            root.join("native/product.so").canonicalize().unwrap()
        );
        assert_eq!(
            product
                .selected_artifacts(ProductLoader::CoreClr)
                .unwrap()
                .0,
            root.join("coreclr/product.dll").canonicalize().unwrap()
        );
        let entries = product.browser_entries().expect("Product UI stages");
        assert!(entries
            .iter()
            .any(|entry| entry.path() == "product-ui/main.js"));
        assert!(entries
            .iter()
            .any(|entry| entry.path() == "product-bootstrap.json"));
        let bootstrap = entries
            .iter()
            .find(|entry| entry.path() == "product-bootstrap.json")
            .expect("bootstrap is staged");
        let bootstrap: serde_json::Value =
            serde_json::from_slice(bootstrap.bytes()).expect("bootstrap is JSON");
        assert_eq!(
            bootstrap["uiProjection"]["expectedStream"],
            "fixture.terrain"
        );
        assert_eq!(
            bootstrap["uiProjection"]["expectedContract"],
            "fixture.terrain.v1"
        );
        assert_eq!(
            bootstrap["renderer"],
            serde_json::json!({ "output": "stream" })
        );
        assert_eq!(product.default_lights(), (true, true));
        assert_eq!(bootstrap["input"]["cursorMode"], "pointer-lock");
        assert!(!entries
            .iter()
            .any(|entry| entry.path().contains("trial.txt")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_packed_product_reads_like_the_loose_one_with_native_code_beside_it() {
        let root = fixture_root("packed");
        write_manifest(&root, "native/product.so");
        let release = root.with_extension("release");
        let report = product_container::pack_product(&root, &release, false).unwrap();
        assert_eq!(
            report.loose_files,
            [
                "coreclr/product.dll",
                "coreclr/product.runtimeconfig.json",
                "native/product.so"
            ]
        );
        let loose = read(&root).unwrap();
        let packed = read(&report.container).unwrap();
        assert!(packed.source.container().is_some());
        assert_eq!(
            packed.selected_artifacts(ProductLoader::CoreClr).unwrap().0,
            release.join("coreclr/product.dll").canonicalize().unwrap()
        );
        let entries = |product: &ProductBundle| {
            product
                .browser_entries()
                .unwrap()
                .into_iter()
                .map(|entry| (entry.path().to_owned(), entry.bytes().to_vec()))
                .collect::<Vec<_>>()
        };
        assert_eq!(entries(&loose), entries(&packed));
        assert_eq!(packed.content_root, "content");
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(release).unwrap();
    }

    #[test]
    fn reports_the_exact_manifest_path_field_that_escapes() {
        let root = fixture_root("escape");
        write_manifest(&root, "../product.so");
        let error = read(&root).expect_err("escaping module is rejected");
        assert!(error.contains("product.json:nativeAot.module"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stages_the_product_selected_unlocked_gameplay_cursor() {
        let root = fixture_root("unlocked-cursor");
        write_manifest(&root, "native/product.so");
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("\"input\":{", "\"input\":{\"cursorMode\":\"unlocked\",");
        fs::write(&manifest_path, manifest).unwrap();

        let bootstrap = read(&root)
            .expect("unlocked cursor mode admits")
            .browser_entries()
            .expect("browser bootstrap stages")
            .into_iter()
            .find(|entry| entry.path() == PRODUCT_HOST_BOOTSTRAP_PATH)
            .expect("browser bootstrap exists");
        let bootstrap: serde_json::Value = serde_json::from_slice(bootstrap.bytes()).unwrap();
        assert_eq!(bootstrap["input"]["cursorMode"], "unlocked");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_input_cursor_modes_before_staging() {
        let root = fixture_root("invalid-cursor-mode");
        write_manifest(&root, "native/product.so");
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let manifest = fs::read_to_string(&manifest_path)
            .unwrap()
            .replace("\"input\":{", "\"input\":{\"cursorMode\":\"freeform\",");
        fs::write(&manifest_path, manifest).unwrap();

        let error = read(&root).expect_err("invalid cursor mode rejects");
        assert!(error.contains("product.json:input.cursorMode"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_manifest_selects_render_and_audio_output() {
        let root = fixture_root("output-selection");
        write_manifest(&root, "native/product.so");
        let bundle = read(&root).expect("default outputs admit");
        assert_eq!(
            bundle.render_output,
            csharp_product_runtime::RenderOutput::Stream
        );
        assert!(!bundle.audio_device_required);

        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&manifest_path).unwrap();
        let with = |renderer: &str, audio: &str| {
            original.replace(
                "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}",
                &format!("\"renderer\":{{\"output\":\"{renderer}\"}},\"audio\":{{\"output\":\"{audio}\"}}"),
            )
        };
        fs::write(&manifest_path, with("window", "device-required")).unwrap();
        let bundle = read(&root).expect("window output admits");
        assert_eq!(
            bundle.render_output,
            csharp_product_runtime::RenderOutput::Window
        );
        assert!(bundle.audio_device_required);

        fs::write(&manifest_path, with("tv", "device-required")).unwrap();
        let error = read(&root).expect_err("unknown output rejects");
        assert!(error.contains("product.json:renderer.output"));
        fs::write(&manifest_path, with("stream", "speakers")).unwrap();
        let error = read(&root).expect_err("unknown audio output rejects");
        assert!(error.contains("product.json:audio.output"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_default_light_modes_at_admission() {
        let root = fixture_root("invalid-default-lighting");
        write_manifest(&root, "native/product.so");
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
            "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}",
            "\"renderer\":{\"lighting\":{\"defaultLights\":{\"world\":\"lantern\",\"viewmodel\":\"disabled\"}}}",
        );
        fs::write(&manifest_path, manifest).unwrap();

        let error = read(&root).expect_err("invalid world lights reject admission");
        assert!(error.contains("product.json:renderer.lighting.defaultLights.world"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_world_and_viewmodel_default_lights_independently() {
        let root = fixture_root("independent-default-lighting");
        write_manifest(&root, "native/product.so");
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
            "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}",
            "\"renderer\":{\"lighting\":{\"defaultLights\":{\"world\":\"disabled\",\"viewmodel\":\"neutral\"}}}",
        );
        fs::write(&manifest_path, manifest).unwrap();

        let product = read(&root).expect("independent light modes admit");
        assert_eq!(product.default_lights(), (false, true));
        assert!(!product.shadows_enabled());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reads_and_validates_scene_shadow_selection() {
        let root = fixture_root("scene-shadows");
        write_manifest(&root, "native/product.so");
        let path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&path).unwrap();
        let marker = "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}";
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"shadows\":\"enabled\"}}",
            ),
        )
        .unwrap();
        assert!(read(&root).unwrap().shadows_enabled());
        fs::write(
            &path,
            original.replace(marker, "\"renderer\":{\"lighting\":{\"shadows\":\"auto\"}}"),
        )
        .unwrap();
        assert!(read(&root)
            .unwrap_err()
            .contains("renderer.lighting.shadows"));
        fs::remove_dir_all(root).unwrap();
    }
}
