//! V1 loose Product directory admission for the packaged Rusty host.
//!
//! This is deployment metadata, not a product callback protocol.  It selects
//! exact staged files before the trusted product is loaded and makes the
//! browser host's product UI bytes immutable for the lifetime of the launch.

use std::{
    fs,
    net::Ipv4Addr,
    path::{Component, Path, PathBuf},
};

use product_dev_host::{ProductDevBundleEntry, ProductDevRendererResource};
use runtime_input::{CompiledInputMappings, DirectInputIntentDescriptor, RuntimeInputMapping};
use runtime_lifecycle::{
    validate_runtime_identity, RealtimeLifecycleConfig, RuntimeLifecycleConfig,
};
use serde::{Deserialize, Serialize};

use super::{content_type, parse_direct_intent, parse_physical_mapping, ProductLoader};

pub(super) const PRODUCT_MANIFEST_NAME: &str = "product.json";
const PRODUCT_ARTIFACT: &str = "rusty.product.bundle";
const PRODUCT_UI_PREFIX: &str = "product-ui";
const PRODUCT_BOOTSTRAP_PATH: &str = "product-bootstrap.json";

#[derive(Debug)]
pub(super) struct ProductBundle {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) native_module: Option<PathBuf>,
    pub(super) coreclr_assembly: Option<PathBuf>,
    pub(super) coreclr_runtimeconfig: Option<PathBuf>,
    pub(super) content_root: PathBuf,
    ui_root: PathBuf,
    ui_entry: String,
    ui_projection: Option<ProductUiProjection>,
    pub(super) lifecycle: RuntimeLifecycleConfig,
    pub(super) lifecycle_mode: &'static str,
    pub(super) direct_intents: Vec<DirectInputIntentDescriptor>,
    pub(super) physical_mappings: Vec<RuntimeInputMapping>,
    pub(super) bind_host: Ipv4Addr,
    pub(super) port: u16,
    pub(super) live_debug: bool,
}

impl ProductBundle {
    pub(super) fn read(root: &Path) -> Result<Self, String> {
        let root = canonical_directory(root, "product")?;
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let bytes = read_regular_file(&root, &manifest_path, "manifest")?;
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
            .map(|native| resolve_regular_file(&root, &native.module, "nativeAot.module"))
            .transpose()?;
        let (coreclr_assembly, coreclr_runtimeconfig) = match manifest.coreclr {
            Some(coreclr) => (
                Some(resolve_regular_file(
                    &root,
                    &coreclr.assembly,
                    "coreclr.assembly",
                )?),
                Some(resolve_regular_file(
                    &root,
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

        let ui_root = resolve_directory(&root, &manifest.ui.root, "ui.root")?;
        let ui_entry_path = resolve_regular_file(&ui_root, &manifest.ui.entry, "ui.entry")?;
        let ui_entry = normalized_relative(&ui_root, &ui_entry_path, "ui.entry")?;
        let assets = resolve_directory(&ui_root, &manifest.ui.assets, "ui.assets")?;
        // Validate the declared assets path even though the complete UI root is
        // staged.  Modules can import adjacent assets without a second UI file
        // vocabulary, while this retains an explicit assets declaration.
        let _ = assets;
        let content_root = resolve_directory(&root, &manifest.content.root, "content.root")?;
        let ui_projection = manifest
            .ui_projection
            .map(ProductUiProjection::from_manifest)
            .transpose()?;

        let (lifecycle, lifecycle_mode) = lifecycle(&manifest.lifecycle)?;
        let (direct_intents, physical_mappings) = input(&manifest.input)?;
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
            content_root,
            ui_root,
            ui_entry,
            ui_projection,
            lifecycle,
            lifecycle_mode,
            direct_intents,
            physical_mappings,
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

    pub(super) fn browser_entries(
        &self,
        resources: &[ProductDevRendererResource],
    ) -> Result<Vec<ProductDevBundleEntry>, String> {
        let mut entries = Vec::new();
        collect_ui(&self.ui_root, &self.ui_root, &mut entries)?;
        let bootstrap = ProductBootstrap {
            artifact: "rusty.product.browser-bootstrap",
            schema_version: 1,
            product: ProductBootstrapIdentity {
                id: &self.id,
                title: &self.title,
            },
            ui: ProductBootstrapUi {
                entry: &format!("{PRODUCT_UI_PREFIX}/{}", self.ui_entry),
            },
            lifecycle: ProductBootstrapLifecycle {
                mode: self.lifecycle_mode,
            },
            ui_projection: self.ui_projection.as_ref(),
        };
        entries.push(
            ProductDevBundleEntry::new(
                PRODUCT_BOOTSTRAP_PATH,
                "application/json; charset=utf-8",
                serde_json::to_vec(&bootstrap).expect("fixed Product browser bootstrap encodes"),
            )
            .map_err(|error| error.to_string())?,
        );
        entries.extend(
            product_dev_host::product_dev_renderer_preload_entries(resources)
                .map_err(|error| error.to_string())?,
        );
        Ok(entries)
    }
}

fn lifecycle(value: &ManifestLifecycle) -> Result<(RuntimeLifecycleConfig, &'static str), String> {
    match value.mode.as_str() {
        "realtime" => {
            let fixed_step = value.fixed_step.as_ref().ok_or_else(|| {
                field_error("lifecycle.fixedStep", "is required for realtime mode")
            })?;
            let config = RealtimeLifecycleConfig::new(fixed_step.hz, fixed_step.max_catch_up_steps)
                .map_err(|error| field_error("lifecycle.fixedStep", error.to_string()))?;
            Ok((RuntimeLifecycleConfig::Realtime(config), "realtime"))
        }
        "demand" => {
            if value.fixed_step.is_some() {
                return Err(field_error(
                    "lifecycle.fixedStep",
                    "is valid only for realtime mode",
                ));
            }
            Ok((RuntimeLifecycleConfig::Demand, "demand"))
        }
        "external" => {
            if value.fixed_step.is_some() {
                return Err(field_error(
                    "lifecycle.fixedStep",
                    "is valid only for realtime mode",
                ));
            }
            Ok((RuntimeLifecycleConfig::External, "external"))
        }
        _ => Err(field_error(
            "lifecycle.mode",
            "must be realtime, demand, or external",
        )),
    }
}

fn input(
    value: &ManifestInput,
) -> Result<(Vec<DirectInputIntentDescriptor>, Vec<RuntimeInputMapping>), String> {
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
    Ok((direct_intents, physical_mappings))
}

fn canonical_directory(path: &Path, field: &str) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        field_error(
            field,
            format!("could not read `{}`: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(field_error(field, "must be a directory, not a symlink"));
    }
    fs::canonicalize(path).map_err(|error| field_error(field, error.to_string()))
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

fn resolve_regular_file(root: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    let path = root.join(relative_path(value, field)?);
    read_regular_file(root, &path, field)?;
    fs::canonicalize(path).map_err(|error| field_error(field, error.to_string()))
}

fn read_regular_file(root: &Path, path: &Path, field: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
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
    fs::read(path).map_err(|error| field_error(field, error.to_string()))
}

fn resolve_directory(root: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    let path = root.join(relative_path(value, field)?);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        field_error(
            field,
            format!("could not read `{}`: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(field_error(field, "must be a directory, not a symlink"));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|error| field_error(field, error.to_string()))?;
    if !canonical.starts_with(root) {
        return Err(field_error(field, "resolved outside Product root"));
    }
    Ok(canonical)
}

fn normalized_relative(root: &Path, path: &Path, field: &str) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|_| field_error(field, "resolved outside its declared root"))?
        .to_str()
        .ok_or_else(|| field_error(field, "must be valid UTF-8"))
        .map(|value| value.replace('\\', "/"))
}

fn collect_ui(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ProductDevBundleEntry>,
) -> Result<(), String> {
    for item in
        fs::read_dir(directory).map_err(|error| field_error("ui.root", error.to_string()))?
    {
        let item = item.map_err(|error| field_error("ui.root", error.to_string()))?;
        let path = item.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| field_error("ui.root", error.to_string()))?;
        if metadata.file_type().is_symlink() {
            return Err(field_error(
                "ui.root",
                format!("contains a symlink: {}", path.display()),
            ));
        }
        if metadata.is_dir() {
            collect_ui(root, &path, entries)?;
        } else if metadata.is_file() {
            let relative = normalized_relative(root, &path, "ui.root")?;
            let media_type = content_type(&relative).ok_or_else(|| {
                field_error(
                    "ui.root",
                    format!("file `{relative}` has no admitted content type"),
                )
            })?;
            entries.push(
                ProductDevBundleEntry::new(
                    format!("{PRODUCT_UI_PREFIX}/{relative}"),
                    media_type,
                    fs::read(path).map_err(|error| field_error("ui.root", error.to_string()))?,
                )
                .map_err(|error| field_error("ui.root", error.to_string()))?,
            );
        } else {
            return Err(field_error("ui.root", "contains a non-regular entry"));
        }
    }
    Ok(())
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
struct ManifestInput {
    #[serde(default)]
    intents: Vec<ManifestIntent>,
    #[serde(default)]
    mappings: Vec<ManifestMapping>,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductBootstrap<'a> {
    artifact: &'static str,
    schema_version: u32,
    product: ProductBootstrapIdentity<'a>,
    ui: ProductBootstrapUi<'a>,
    lifecycle: ProductBootstrapLifecycle,
    #[serde(skip_serializing_if = "Option::is_none")]
    ui_projection: Option<&'a ProductUiProjection>,
}
#[derive(Serialize)]
struct ProductBootstrapIdentity<'a> {
    id: &'a str,
    title: &'a str,
}
#[derive(Serialize)]
struct ProductBootstrapUi<'a> {
    entry: &'a str,
}
#[derive(Serialize)]
struct ProductBootstrapLifecycle {
    mode: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProductBootstrapUiProjection<'a> {
    expected_stream: &'a str,
    expected_contract: &'a str,
}

impl Serialize for ProductUiProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ProductBootstrapUiProjection {
            expected_stream: &self.expected_stream,
            expected_contract: &self.expected_contract,
        }
        .serialize(serializer)
    }
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
        let product = ProductBundle::read(&root).expect("V1 Product bundle admits");
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
        let entries = product.browser_entries(&[]).expect("Product UI stages");
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
        assert!(!entries
            .iter()
            .any(|entry| entry.path().contains("trial.txt")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_the_exact_manifest_path_field_that_escapes() {
        let root = fixture_root("escape");
        write_manifest(&root, "../product.so");
        let error = ProductBundle::read(&root).expect_err("escaping module is rejected");
        assert!(error.contains("product.json:nativeAot.module"));
        fs::remove_dir_all(root).unwrap();
    }
}
