//! V1 staged Product admission for the packaged Rusty host, from a loose
//! directory or a container (`product_container::ProductSource`).
//!
//! This is deployment metadata, not a product callback protocol.  It selects
//! exact staged files before the trusted product is loaded and makes the
//! browser host's product UI bytes immutable for the lifetime of the launch.

use std::{
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use csharp_product_runtime::ProductSource;
use product_container::{is_relative_path, join};

use csharp_engine_abi::NativeInputCursorMode;
use product_host::{
    ProductHostBootstrapInput, ProductHostBootstrapProduct, ProductHostBootstrapRenderer,
    ProductHostBootstrapUi, ProductHostBootstrapUiProjection, ProductHostBrowserBootstrap,
    ProductHostBundleEntry, ProductHostCursorMode, ProductHostPresentationAspect,
    PRODUCT_HOST_BOOTSTRAP_PATH,
};
use render_model::{AmbientOcclusionMode, AmbientOcclusionSettings, RendererSettingsDescriptor};
use runtime_input::{DirectInputIntentDescriptor, RuntimeInputMapping};
use runtime_lifecycle::RuntimeLifecycleConfig;
use serde::Deserialize;

use super::{content_type, parse_direct_intent, parse_physical_mapping, ProductLoader};

pub(super) const PRODUCT_MANIFEST_NAME: &str = "product.json";
const PRODUCT_ARTIFACT: &str = "rusty.product.bundle";
const PRODUCT_UI_PREFIX: &str = "product-ui";
const PRODUCT_CONTENT_PREFIX: &str = "product-content";

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
    /// Content subtrees, relative to the content root, the page may read.
    ui_content: Vec<String>,
    ui_entry: String,
    ui_projection: Option<ManifestUiProjection>,
    renderer_settings: ProductRendererSettings,
    /// Where the runtime draws (`renderer.output`).
    pub(super) render_output: csharp_product_runtime::RenderOutput,
    /// The web engine drawing the UI page in the window (`renderer.windowUi`).
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(super) window_ui: WindowUi,
    /// The aspect range the page keeps (`renderer.presentationAspect`).
    presentation_aspect: Option<ProductHostPresentationAspect>,
    /// Where committed audio plays (`audio.output`).
    pub(super) audio_output: csharp_product_runtime::AudioOutputSelection,
    pub(super) lifecycle: RuntimeLifecycleConfig,
    pub(super) direct_intents: Vec<DirectInputIntentDescriptor>,
    pub(super) physical_mappings: Vec<RuntimeInputMapping>,
    pub(super) input_cursor_mode: ProductInputCursorMode,
    pub(super) bind_host: Ipv4Addr,
    pub(super) port: u16,
    pub(super) live_debug: bool,
    /// The source content root a `rusty dev` staging named.
    pub(super) content_source: Option<PathBuf>,
    /// Where a relative `OpenContainer` path resolves: the directory a
    /// `rusty dev` staging named, else the Product's own directory (beside
    /// `product.rpak` in a release).
    pub(super) installed_content: PathBuf,
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
        let native_module = manifest
            .native_aot
            .map(|native| native_artifact(root, &native.module, "nativeAot.module"))
            .transpose()?;
        let (coreclr_assembly, coreclr_runtimeconfig) = match manifest.coreclr {
            Some(coreclr) => (
                Some(native_artifact(
                    root,
                    &coreclr.assembly,
                    "coreclr.assembly",
                )?),
                Some(native_artifact(
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
        let content_root = product_directory(source, "", &manifest.content.root, "content.root")?;
        for subtree in &manifest.content.ui {
            product_directory(source, &content_root, subtree, "content.ui")?;
        }
        let render_output = match manifest.renderer.output.as_deref() {
            None | Some("stream") => csharp_product_runtime::RenderOutput::Stream,
            Some("window") => csharp_product_runtime::RenderOutput::Window,
            Some(_) => return Err(field_error("renderer.output", "must be stream or window")),
        };
        let window_ui = match manifest.renderer.window_ui.as_deref() {
            None => WindowUi::Servo,
            Some(name) => WindowUi::parse(name)
                .ok_or_else(|| field_error("renderer.windowUi", "must be chromium or servo"))?,
        };
        let presentation_aspect = match manifest.renderer.presentation_aspect {
            None => None,
            Some(ManifestPresentationAspect { minimum, maximum })
                if minimum.is_finite()
                    && maximum.is_finite()
                    && 0.0 < minimum
                    && minimum <= maximum =>
            {
                Some(ProductHostPresentationAspect { minimum, maximum })
            }
            Some(_) => {
                return Err(field_error(
                    "renderer.presentationAspect",
                    "minimum and maximum must be finite, positive and in order",
                ))
            }
        };
        // Sound goes where the frames go unless the product chooses.
        use csharp_product_runtime::{AudioOutputSelection, RenderOutput};
        let audio_output = match (manifest.audio.output.as_deref(), render_output) {
            (None, RenderOutput::Stream) | (Some("stream"), RenderOutput::Stream) => {
                AudioOutputSelection::Stream
            }
            (Some("stream"), RenderOutput::Window) => {
                return Err(field_error(
                    "audio.output",
                    "stream needs renderer.output stream; a window plays on its device",
                ))
            }
            (None, RenderOutput::Window) | (Some("device-optional"), _) => {
                AudioOutputSelection::Device { required: false }
            }
            (Some("device-required"), _) => AudioOutputSelection::Device { required: true },
            (Some(_), _) => {
                return Err(field_error(
                    "audio.output",
                    "must be stream, device-optional or device-required",
                ))
            }
        };
        let renderer_settings = ProductRendererSettings::from_manifest(manifest.renderer)?;

        let lifecycle = lifecycle(&manifest.lifecycle)?;
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
            ui_content: manifest.content.ui,
            ui_entry,
            ui_projection: manifest.ui_projection,
            renderer_settings,
            render_output,
            window_ui,
            presentation_aspect,
            audio_output,
            lifecycle,
            direct_intents,
            physical_mappings,
            input_cursor_mode,
            bind_host,
            port: manifest.server.port,
            live_debug: manifest.server.live_debug,
            installed_content: manifest.development.as_ref().map_or_else(
                || source.native_root().to_owned(),
                |development| development.installed_content.clone(),
            ),
            content_source: manifest
                .development
                .map(|development| development.content_source),
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

    /// Points the CoreCLR artifacts at a private copy of their directory.
    #[cfg(windows)]
    pub(super) fn load_coreclr_from_private_copy(&mut self) -> Result<(), String> {
        let (Some(assembly), Some(runtimeconfig)) = (
            self.coreclr_assembly.clone(),
            self.coreclr_runtimeconfig.clone(),
        ) else {
            return Ok(());
        };
        let directory = assembly
            .parent()
            .ok_or("PRODUCT_HOST_RUNTIME_COPY: the CoreCLR assembly has no directory")?;
        let copy = crate::platform::private_loader_copy(directory)?;
        let rebase = |path: &Path| match path.strip_prefix(directory) {
            Ok(relative) => copy.join(relative),
            Err(_) => path.to_owned(),
        };
        self.coreclr_assembly = Some(rebase(&assembly));
        self.coreclr_runtimeconfig = Some(rebase(&runtimeconfig));
        Ok(())
    }

    /// The manifest's world and viewmodel default light rigs, on or off.
    pub(super) fn default_lights(&self) -> (bool, bool) {
        self.renderer_settings.enabled()
    }

    /// The manifest's renderer settings: the renderer's initial values,
    /// which `RendererSettings` changes at runtime.
    pub(super) fn renderer_settings(&self) -> RendererSettingsDescriptor {
        self.renderer_settings.settings
    }

    pub(super) fn browser_entries(&self) -> Result<Vec<ProductHostBundleEntry>, String> {
        let mut entries = Vec::new();
        self.collect_ui(&mut entries)?;
        self.collect_ui_content(&mut entries)?;
        let bootstrap = ProductHostBrowserBootstrap {
            product: ProductHostBootstrapProduct {
                id: self.id.clone(),
                title: self.title.clone(),
            },
            ui: ProductHostBootstrapUi {
                entry: format!("{PRODUCT_UI_PREFIX}/{}", self.ui_entry),
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
                presentation_aspect: self.presentation_aspect,
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

fn lifecycle(value: &ManifestLifecycle) -> Result<RuntimeLifecycleConfig, String> {
    RuntimeLifecycleConfig::new(value.fixed_step.hz, value.fixed_step.max_catch_up_steps)
        .map_err(|error| field_error("lifecycle.fixedStep", error.to_string()))
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
    Ok((
        direct_intents,
        physical_mappings,
        ProductInputCursorMode::parse(value.cursor_mode.as_deref())?,
    ))
}

/// A native artifact under `root`, loaded by path: hostfxr and the dynamic
/// loader report a missing or unloadable file, and a symlink loads as its
/// target.
fn native_artifact(root: &Path, value: &str, field: &str) -> Result<PathBuf, String> {
    if !is_relative_path(value) {
        return Err(field_error(
            field,
            "must be a non-empty relative non-escaping path",
        ));
    }
    Ok(root.join(value))
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
        return Err(field_error(field, "must be a directory"));
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
        return Err(field_error(field, "must be a regular file"));
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
            let bytes = self.source.read(&path).map_err(ui_error)?;
            entries.push(
                ProductHostBundleEntry::new(
                    format!("{PRODUCT_UI_PREFIX}/{relative}"),
                    content_type(relative),
                    bytes.into_owned(),
                )
                .map_err(|error| field_error("ui.root", error.to_string()))?,
            );
        }
        Ok(())
    }

    /// The content subtrees the product opened to its UI, at
    /// `product-content/<path within the content root>`. Overlapping subtrees
    /// serve each file once.
    fn collect_ui_content(&self, entries: &mut Vec<ProductHostBundleEntry>) -> Result<(), String> {
        let content_error = |error| field_error("content.ui", error);
        let mut paths = std::collections::BTreeSet::new();
        for subtree in &self.ui_content {
            paths.extend(
                self.source
                    .files(&join(&self.content_root, subtree))
                    .map_err(content_error)?,
            );
        }
        for path in paths {
            let relative = &path[self.content_root.len() + 1..];
            let bytes = self.source.read(&path).map_err(content_error)?;
            entries.push(
                ProductHostBundleEntry::new(
                    format!("{PRODUCT_CONTENT_PREFIX}/{relative}"),
                    content_type(relative),
                    bytes.into_owned(),
                )
                .map_err(|error| field_error("content.ui", error.to_string()))?,
            );
        }
        Ok(())
    }
}

/// The web engine drawing the UI page in the desktop window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowUi {
    Chromium,
    Servo,
}

impl WindowUi {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "chromium" => Some(Self::Chromium),
            "servo" => Some(Self::Servo),
            _ => None,
        }
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
    development: Option<ManifestDevelopment>,
}

/// Written only by `rusty dev` staging.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestDevelopment {
    content_source: PathBuf,
    installed_content: PathBuf,
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
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestUiProjection {
    expected_stream: String,
    expected_contract: String,
}
#[derive(Debug, Deserialize)]
struct ManifestContent {
    root: String,
    /// Content subtrees the product UI may read.
    #[serde(default)]
    ui: Vec<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRenderer {
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    window_ui: Option<String>,
    #[serde(default)]
    presentation_aspect: Option<ManifestPresentationAspect>,
    #[serde(default)]
    lighting: ManifestRendererLighting,
    #[serde(default)]
    gpu_culling: Option<String>,
    #[serde(default)]
    antialiasing: Option<String>,
    #[serde(default)]
    vsync: Option<String>,
    #[serde(default)]
    render_scale: Option<f32>,
}
#[derive(Debug, Clone, Copy, Deserialize)]
struct ManifestPresentationAspect {
    minimum: f64,
    maximum: f64,
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
    /// `enabled` or `disabled`; absent is enabled.
    #[serde(default)]
    backdrop_shadows: Option<String>,
    /// Shadow layers at once; 0 or absent for no limit.
    #[serde(default)]
    shadow_budget: Option<u32>,
    #[serde(default)]
    ambient_occlusion: Option<ManifestAmbientOcclusion>,
    #[serde(default)]
    clustered_lighting: Option<String>,
    /// `off`, `low` or `high`; absent is off.
    #[serde(default)]
    volumetric_fog: Option<String>,
    /// `off`, `low` or `high`; absent is off.
    #[serde(default)]
    volumetric_clouds: Option<String>,
    /// `off`, `low` or `high`; absent is off.
    #[serde(default)]
    water_reflections: Option<String>,
    #[serde(default)]
    default_lights: ManifestDefaultLights,
}
#[derive(Debug, Deserialize)]
struct ManifestAmbientOcclusion {
    mode: String,
    #[serde(default = "full_strength")]
    strength: f32,
    #[serde(default = "default_radius")]
    radius: f32,
}
fn full_strength() -> f32 {
    AmbientOcclusionSettings::DEFAULT.strength
}
fn default_radius() -> f32 {
    AmbientOcclusionSettings::DEFAULT.radius
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
/// The manifest's renderer settings and default light rigs.
#[derive(Debug)]
struct ProductRendererSettings {
    settings: RendererSettingsDescriptor,
    world: ProductDefaultLights,
    viewmodel: ProductDefaultLights,
}
impl ProductRendererSettings {
    /// Whether the world and viewmodel default rigs are on.
    fn enabled(&self) -> (bool, bool) {
        (
            matches!(self.world, ProductDefaultLights::Neutral),
            matches!(self.viewmodel, ProductDefaultLights::Neutral),
        )
    }

    fn from_manifest(value: ManifestRenderer) -> Result<Self, String> {
        let switch = |value: Option<&str>, field: &str| match value {
            None | Some("disabled") => Ok(false),
            Some("enabled") => Ok(true),
            Some(_) => Err(field_error(field, "must be enabled or disabled")),
        };
        let shadows = switch(
            value.lighting.shadows.as_deref(),
            "renderer.lighting.shadows",
        )?;
        let backdrop_shadows = match value.lighting.backdrop_shadows.as_deref() {
            None | Some("enabled") => true,
            Some("disabled") => false,
            Some(_) => {
                return Err(field_error(
                    "renderer.lighting.backdropShadows",
                    "must be enabled or disabled",
                ))
            }
        };
        let ambient_occlusion = match value.lighting.ambient_occlusion {
            None => AmbientOcclusionSettings::DEFAULT,
            Some(ambient_occlusion) => {
                let mode = match ambient_occlusion.mode.as_str() {
                    "disabled" => AmbientOcclusionMode::Disabled,
                    "enabled" => AmbientOcclusionMode::ScreenSpace,
                    "distanceField" => AmbientOcclusionMode::DistanceField,
                    _ => {
                        return Err(field_error(
                            "renderer.lighting.ambientOcclusion.mode",
                            "must be enabled, distanceField or disabled",
                        ))
                    }
                };
                if !ambient_occlusion.strength.is_finite() || ambient_occlusion.strength < 0.0 {
                    return Err(field_error(
                        "renderer.lighting.ambientOcclusion.strength",
                        "must be a finite non-negative number",
                    ));
                }
                if !ambient_occlusion.radius.is_finite() || ambient_occlusion.radius <= 0.0 {
                    return Err(field_error(
                        "renderer.lighting.ambientOcclusion.radius",
                        "must be a finite positive number of world units",
                    ));
                }
                AmbientOcclusionSettings {
                    mode,
                    strength: ambient_occlusion.strength,
                    radius: ambient_occlusion.radius,
                }
            }
        };
        let antialiasing = match value.antialiasing.as_deref() {
            None => render_model::Antialiasing::Msaa4,
            Some(name) => render_model::Antialiasing::from_name(name).ok_or_else(|| {
                field_error(
                    "renderer.antialiasing",
                    "must be off, 2x, 4x, temporal or dlss",
                )
            })?,
        };
        let vsync = match value.vsync.as_deref() {
            None | Some("enabled") => true,
            Some("disabled") => false,
            Some(_) => return Err(field_error("renderer.vsync", "must be enabled or disabled")),
        };
        let volumetric_fog = match value.lighting.volumetric_fog.as_deref() {
            None | Some("off") => render_model::VolumetricFogQuality::Off,
            Some("low") => render_model::VolumetricFogQuality::Low,
            Some("high") => render_model::VolumetricFogQuality::High,
            Some(_) => {
                return Err(field_error(
                    "renderer.lighting.volumetricFog",
                    "must be off, low or high",
                ))
            }
        };
        let volumetric_clouds = match value.lighting.volumetric_clouds.as_deref() {
            None | Some("off") => render_model::VolumetricCloudsQuality::Off,
            Some("low") => render_model::VolumetricCloudsQuality::Low,
            Some("high") => render_model::VolumetricCloudsQuality::High,
            Some(_) => {
                return Err(field_error(
                    "renderer.lighting.volumetricClouds",
                    "must be off, low or high",
                ))
            }
        };
        let water_reflections = match value.lighting.water_reflections.as_deref() {
            None | Some("off") => render_model::WaterReflectionsQuality::Off,
            Some("low") => render_model::WaterReflectionsQuality::Low,
            Some("high") => render_model::WaterReflectionsQuality::High,
            Some(_) => {
                return Err(field_error(
                    "renderer.lighting.waterReflections",
                    "must be off, low or high",
                ))
            }
        };
        let render_scale = value.render_scale.unwrap_or(1.0);
        if !render_scale.is_finite()
            || !(RendererSettingsDescriptor::MIN_RENDER_SCALE..=1.0).contains(&render_scale)
        {
            return Err(field_error(
                "renderer.renderScale",
                "must be a number from 0.33 to 1",
            ));
        }
        Ok(Self {
            settings: RendererSettingsDescriptor {
                shadows,
                backdrop_shadows,
                shadow_budget: value.lighting.shadow_budget.filter(|&budget| budget > 0),
                ambient_occlusion,
                antialiasing,
                render_scale,
                vsync,
                clustered_lighting: switch(
                    value.lighting.clustered_lighting.as_deref(),
                    "renderer.lighting.clusteredLighting",
                )?,
                gpu_culling: switch(value.gpu_culling.as_deref(), "renderer.gpuCulling")?,
                // Products turn it on through `RendererSettings.Set`.
                occlusion_culling: false,
                volumetric_fog,
                volumetric_clouds,
                water_reflections,
            },
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
    fixed_step: ManifestFixedStep,
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
    Confined,
}
impl ProductInputCursorMode {
    fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("pointer-lock") => Ok(Self::PointerLock),
            Some("unlocked") => Ok(Self::Unlocked),
            Some("confined") => Ok(Self::Confined),
            Some(_) => Err(field_error(
                "input.cursorMode",
                "must be pointer-lock, unlocked or confined",
            )),
        }
    }

    pub(super) fn native(self) -> NativeInputCursorMode {
        match self {
            Self::PointerLock => NativeInputCursorMode::PointerLock,
            Self::Unlocked => NativeInputCursorMode::Unlocked,
            Self::Confined => NativeInputCursorMode::Confined,
        }
    }
    fn bootstrap(self) -> ProductHostCursorMode {
        match self {
            Self::PointerLock => ProductHostCursorMode::PointerLock,
            Self::Unlocked => ProductHostCursorMode::Unlocked,
            Self::Confined => ProductHostCursorMode::Confined,
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
          "ui":{{"root":"ui","entry":"main.js"}},
          "content":{{"root":"content"}},
          "uiProjection":{{"expectedStream":"fixture.terrain","expectedContract":"fixture.terrain.v1"}},
          "lifecycle":{{"fixedStep":{{"hz":60,"maxCatchUpSteps":2}}}},
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
            root.canonicalize().unwrap().join("native/product.so")
        );
        assert_eq!(
            product
                .selected_artifacts(ProductLoader::CoreClr)
                .unwrap()
                .0,
            root.canonicalize().unwrap().join("coreclr/product.dll")
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
    fn serves_every_ui_root_file_with_the_type_of_its_extension() {
        let root = fixture_root("ui-types");
        write_manifest(&root, "native/product.so");
        fs::write(root.join("ui/assets/skin.ttf"), b"font").unwrap();
        fs::write(root.join("ui/assets/Icon@2x.PNG"), b"png").unwrap();
        fs::write(root.join("ui/assets/notes.unknown"), b"notes").unwrap();

        let entries = read(&root).unwrap().browser_entries().unwrap();
        let served = |path: &str| {
            entries
                .iter()
                .find(|entry| entry.path() == path)
                .map(|entry| entry.content_type().to_owned())
        };
        assert_eq!(
            served("product-ui/assets/skin.ttf").as_deref(),
            Some("font/ttf")
        );
        assert_eq!(
            served("product-ui/assets/Icon@2x.PNG").as_deref(),
            Some("image/png")
        );
        assert_eq!(
            served("product-ui/assets/notes.unknown").as_deref(),
            Some("application/octet-stream")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn serves_only_the_content_subtrees_the_product_opens_to_its_ui() {
        let root = fixture_root("ui-content");
        write_manifest(&root, "native/product.so");
        let manifest = fs::read_to_string(root.join(PRODUCT_MANIFEST_NAME)).unwrap();
        fs::write(
            root.join(PRODUCT_MANIFEST_NAME),
            manifest.replace(
                r#""content":{"root":"content"}"#,
                r#""content":{"root":"content","ui":["supplies/icons","supplies/icons/large"]}"#,
            ),
        )
        .unwrap();
        fs::create_dir_all(root.join("content/supplies/icons/large")).unwrap();
        fs::write(root.join("content/supplies/items.json"), b"[]").unwrap();
        fs::write(root.join("content/supplies/icons/lamp.svg"), b"<svg/>").unwrap();
        fs::write(
            root.join("content/supplies/icons/large/Lamp@2x.PNG"),
            b"png",
        )
        .unwrap();

        let entries = read(&root).unwrap().browser_entries().unwrap();
        let served = |path: &str| {
            entries
                .iter()
                .find(|entry| entry.path() == path)
                .map(|entry| entry.content_type().to_owned())
        };
        assert_eq!(
            served("product-content/supplies/icons/lamp.svg").as_deref(),
            Some("image/svg+xml")
        );
        assert_eq!(
            served("product-content/supplies/icons/large/Lamp@2x.PNG").as_deref(),
            Some("image/png")
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.path().starts_with("product-content/"))
                .count(),
            2
        );

        fs::write(
            root.join(PRODUCT_MANIFEST_NAME),
            manifest.replace(
                r#""content":{"root":"content"}"#,
                r#""content":{"root":"content","ui":["../ui"]}"#,
            ),
        )
        .unwrap();
        assert!(read(&root).unwrap_err().contains("content.ui"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rusty_dev_staging_names_the_installed_content_directory() {
        let root = fixture_root("installed");
        write_manifest(&root, "native/product.so");
        let manifest = fs::read_to_string(root.join(PRODUCT_MANIFEST_NAME)).unwrap();
        fs::write(
            root.join(PRODUCT_MANIFEST_NAME),
            manifest.replace(
                r#""ignoredV1Field":true"#,
                r#""development":{"contentSource":"/project/content","installedContent":"/project/local"}"#,
            ),
        )
        .unwrap();
        let product = read(&root).unwrap();
        assert_eq!(product.installed_content, Path::new("/project/local"));
        assert_eq!(
            product.content_source.as_deref(),
            Some(Path::new("/project/content"))
        );
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
        // Containers installed beside the product resolve beside product.rpak.
        assert_eq!(packed.installed_content, release.canonicalize().unwrap());
        assert_eq!(loose.installed_content, root.canonicalize().unwrap());
        assert_eq!(
            packed.selected_artifacts(ProductLoader::CoreClr).unwrap().0,
            release.canonicalize().unwrap().join("coreclr/product.dll")
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

    #[cfg(unix)]
    #[test]
    fn a_symlinked_native_artifact_admits_at_its_staged_path() {
        let root = fixture_root("symlinked-artifact");
        let built = root.with_extension("built");
        fs::create_dir_all(&built).unwrap();
        fs::write(built.join("product.so"), b"built").unwrap();
        std::os::unix::fs::symlink(built.join("product.so"), root.join("native/linked.so"))
            .unwrap();
        write_manifest(&root, "native/linked.so");
        let product = read(&root).expect("a symlinked module admits");
        let (module, _) = product
            .selected_artifacts(ProductLoader::NativeAot)
            .unwrap();
        assert_eq!(
            module,
            root.canonicalize().unwrap().join("native/linked.so")
        );
        assert_eq!(fs::read(module).unwrap(), b"built");
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(built).unwrap();
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
    fn stages_the_product_selected_gameplay_cursor() {
        for mode in ["unlocked", "confined"] {
            let root = fixture_root(&format!("{mode}-cursor"));
            write_manifest(&root, "native/product.so");
            let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
            let manifest = fs::read_to_string(&manifest_path).unwrap().replace(
                "\"input\":{",
                &format!("\"input\":{{\"cursorMode\":\"{mode}\","),
            );
            fs::write(&manifest_path, manifest).unwrap();

            let bootstrap = read(&root)
                .expect("the cursor mode admits")
                .browser_entries()
                .expect("browser bootstrap stages")
                .into_iter()
                .find(|entry| entry.path() == PRODUCT_HOST_BOOTSTRAP_PATH)
                .expect("browser bootstrap exists");
            let bootstrap: serde_json::Value = serde_json::from_slice(bootstrap.bytes()).unwrap();
            assert_eq!(bootstrap["input"]["cursorMode"], mode);
            fs::remove_dir_all(root).unwrap();
        }
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
    fn the_manifest_declares_the_presentation_aspect_range_for_the_page() {
        let root = fixture_root("presentation-aspect");
        write_manifest(&root, "native/product.so");
        assert_eq!(read(&root).unwrap().presentation_aspect, None);
        let manifest_path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&manifest_path).unwrap();
        let with = |aspect: &str| {
            original.replace(
                "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}",
                &format!("\"renderer\":{{\"presentationAspect\":{aspect}}}"),
            )
        };
        fs::write(&manifest_path, with("{\"minimum\":1.25,\"maximum\":2.4}")).unwrap();
        assert_eq!(
            read(&root).unwrap().presentation_aspect,
            Some(ProductHostPresentationAspect {
                minimum: 1.25,
                maximum: 2.4
            })
        );
        fs::write(&manifest_path, with("{\"minimum\":2,\"maximum\":1}")).unwrap();
        let error = read(&root).expect_err("an inverted range rejects");
        assert!(error.contains("product.json:renderer.presentationAspect"));
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
        assert_eq!(
            bundle.audio_output,
            csharp_product_runtime::AudioOutputSelection::Stream
        );

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
        assert_eq!(
            bundle.audio_output,
            csharp_product_runtime::AudioOutputSelection::Device { required: true }
        );
        fs::write(&manifest_path, with("stream", "device-optional")).unwrap();
        assert_eq!(
            read(&root)
                .expect("a streamed product may keep the device")
                .audio_output,
            csharp_product_runtime::AudioOutputSelection::Device { required: false }
        );
        fs::write(&manifest_path, with("window", "stream")).unwrap();
        let error = read(&root).expect_err("a window does not stream audio");
        assert!(error.contains("product.json:audio.output"));

        fs::write(&manifest_path, with("tv", "device-required")).unwrap();
        let error = read(&root).expect_err("unknown output rejects");
        assert!(error.contains("product.json:renderer.output"));
        assert_eq!(bundle.window_ui, WindowUi::Servo);
        let with_ui = |ui: &str| {
            original.replace(
                "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}",
                &format!("\"renderer\":{{\"output\":\"window\",\"windowUi\":\"{ui}\"}}"),
            )
        };
        fs::write(&manifest_path, with_ui("chromium")).unwrap();
        assert_eq!(
            read(&root).expect("chromium admits").window_ui,
            WindowUi::Chromium
        );
        fs::write(&manifest_path, with_ui("gecko")).unwrap();
        let error = read(&root).expect_err("unknown window UI rejects");
        assert!(error.contains("product.json:renderer.windowUi"));
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
        assert!(!product.renderer_settings().shadows);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reads_and_validates_ambient_occlusion_selection() {
        let root = fixture_root("ambient-occlusion");
        write_manifest(&root, "native/product.so");
        let path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&path).unwrap();
        assert_eq!(
            read(&root).unwrap().renderer_settings().ambient_occlusion,
            AmbientOcclusionSettings::DEFAULT,
            "absent: off"
        );
        let marker = "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}";
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"ambientOcclusion\":{\"mode\":\"enabled\",\"strength\":0.5}}}",
            ),
        )
        .unwrap();
        assert_eq!(
            read(&root).unwrap().renderer_settings().ambient_occlusion,
            AmbientOcclusionSettings {
                mode: AmbientOcclusionMode::ScreenSpace,
                strength: 0.5,
                radius: AmbientOcclusionSettings::DEFAULT.radius,
            }
        );
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"ambientOcclusion\":{\"mode\":\"distanceField\",\"strength\":1}}}",
            ),
        )
        .unwrap();
        assert_eq!(
            read(&root)
                .unwrap()
                .renderer_settings()
                .ambient_occlusion
                .mode,
            AmbientOcclusionMode::DistanceField
        );
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"ambientOcclusion\":{\"mode\":\"always\"}}}",
            ),
        )
        .unwrap();
        assert!(read(&root)
            .unwrap_err()
            .contains("renderer.lighting.ambientOcclusion.mode"));
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"ambientOcclusion\":{\"mode\":\"enabled\",\"radius\":2}}}",
            ),
        )
        .unwrap();
        assert_eq!(
            read(&root)
                .unwrap()
                .renderer_settings()
                .ambient_occlusion
                .radius,
            2.0
        );
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"ambientOcclusion\":{\"mode\":\"enabled\",\"radius\":0}}}",
            ),
        )
        .unwrap();
        assert!(read(&root)
            .unwrap_err()
            .contains("renderer.lighting.ambientOcclusion.radius"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reads_and_validates_antialiasing_and_vsync() {
        let root = fixture_root("antialiasing-vsync");
        write_manifest(&root, "native/product.so");
        let path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&path).unwrap();
        let defaults = read(&root).unwrap().renderer_settings();
        assert_eq!(
            (defaults.antialiasing, defaults.vsync),
            (render_model::Antialiasing::Msaa4, true),
            "absent: 4x, vsync"
        );
        let marker = "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}";
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"antialiasing\":\"2x\",\"vsync\":\"disabled\"}",
            ),
        )
        .unwrap();
        let settings = read(&root).unwrap().renderer_settings();
        assert_eq!(
            (settings.antialiasing, settings.vsync),
            (render_model::Antialiasing::Msaa2, false)
        );
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"antialiasing\":\"temporal\",\"renderScale\":0.33}",
            ),
        )
        .unwrap();
        let settings = read(&root).unwrap().renderer_settings();
        assert_eq!(
            (settings.antialiasing, settings.render_scale),
            (render_model::Antialiasing::Temporal, 0.33)
        );
        fs::write(
            &path,
            original.replace(marker, "\"renderer\":{\"antialiasing\":\"8x\"}"),
        )
        .unwrap();
        assert!(read(&root).unwrap_err().contains("renderer.antialiasing"));
        fs::write(
            &path,
            original.replace(marker, "\"renderer\":{\"vsync\":\"maybe\"}"),
        )
        .unwrap();
        assert!(read(&root).unwrap_err().contains("renderer.vsync"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reads_and_validates_gpu_culling_selection() {
        let root = fixture_root("gpu-culling");
        write_manifest(&root, "native/product.so");
        let path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&path).unwrap();
        assert!(
            !read(&root).unwrap().renderer_settings().gpu_culling,
            "absent: CPU"
        );
        let marker = "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}";
        fs::write(
            &path,
            original.replace(marker, "\"renderer\":{\"gpuCulling\":\"enabled\"}"),
        )
        .unwrap();
        assert!(read(&root).unwrap().renderer_settings().gpu_culling);
        fs::write(
            &path,
            original.replace(marker, "\"renderer\":{\"gpuCulling\":\"maybe\"}"),
        )
        .unwrap();
        assert!(read(&root).unwrap_err().contains("renderer.gpuCulling"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reads_and_validates_clustered_lighting_selection() {
        let root = fixture_root("clustered-lighting");
        write_manifest(&root, "native/product.so");
        let path = root.join(PRODUCT_MANIFEST_NAME);
        let original = fs::read_to_string(&path).unwrap();
        assert!(
            !read(&root).unwrap().renderer_settings().clustered_lighting,
            "absent: loop"
        );
        let marker = "\"uiProjection\":{\"expectedStream\":\"fixture.terrain\",\"expectedContract\":\"fixture.terrain.v1\"}";
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"clusteredLighting\":\"enabled\"}}",
            ),
        )
        .unwrap();
        assert!(read(&root).unwrap().renderer_settings().clustered_lighting);
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"clusteredLighting\":\"sometimes\"}}",
            ),
        )
        .unwrap();
        assert!(read(&root)
            .unwrap_err()
            .contains("renderer.lighting.clusteredLighting"));
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
        let product = read(&root).unwrap();
        assert!(product.renderer_settings().shadows);
        assert_eq!(product.renderer_settings().shadow_budget, None);
        fs::write(
            &path,
            original.replace(
                marker,
                "\"renderer\":{\"lighting\":{\"shadows\":\"enabled\",\"shadowBudget\":24}}",
            ),
        )
        .unwrap();
        assert_eq!(
            read(&root).unwrap().renderer_settings().shadow_budget,
            Some(24)
        );
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
