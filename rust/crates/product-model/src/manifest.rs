use std::collections::BTreeSet;

use serde::Deserialize;

use crate::{diagnostic::failure, ProductModelError, ProductPath};

const SOURCE: &str = "rusty.toml";
pub const MAX_PRODUCT_MANIFEST_BYTES: usize = 65_536;
pub const MAX_IDENTITY_BYTES: usize = 128;
pub const MAX_COMPOSITION_ENTRYPOINTS: usize = 32;
pub const MAX_WRAPPERS: usize = 8;
pub const MAX_WRAPPER_PERMISSIONS: usize = 64;
/// Highest fixed-step rate admitted by the current Product Manifest.
pub const MAX_REALTIME_HZ: u32 = 240;
/// Highest bounded realtime catch-up batch admitted by the current Product Manifest.
pub const MAX_REALTIME_CATCH_UP_STEPS: u32 = 16;
const MIN_WINDOW_DIMENSION: u32 = 320;
const MAX_WINDOW_DIMENSION: u32 = 8_192;
const MAX_WRAPPER_TITLE_BYTES: usize = 128;
/// Maximum UTF-8 byte length admitted for a wrapper's semantic version.
pub const MAX_WRAPPER_VERSION_BYTES: usize = 64;

/// The only lifecycle selections admitted by the current product model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LifecycleMode {
    Realtime,
    Demand,
    External,
}

/// Bounded clock configuration used only by the realtime lifecycle mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealtimeClock {
    fixed_step_hz: u32,
    max_catch_up_steps: u32,
}

impl RealtimeClock {
    /// Creates candidate clock settings. Bounds are checked with the lifecycle
    /// when the enclosing manifest is validated.
    pub const fn new(fixed_step_hz: u32, max_catch_up_steps: u32) -> Self {
        Self {
            fixed_step_hz,
            max_catch_up_steps,
        }
    }

    pub fn fixed_step_hz(&self) -> u32 {
        self.fixed_step_hz
    }

    pub fn max_catch_up_steps(&self) -> u32 {
        self.max_catch_up_steps
    }
}

/// A downstream-owned wrapper declaration. The crate does not build or run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WrapperKind {
    Tauri,
    Electron,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseChannel {
    Development,
    Preview,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperDeclaration {
    id: String,
    kind: WrapperKind,
    version: String,
    application_id: String,
    title: String,
    window_width: u32,
    window_height: u32,
    resizable: bool,
    permissions: Vec<String>,
    storage_namespace: String,
    release_channel: ReleaseChannel,
    singleton: bool,
}

impl WrapperDeclaration {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn kind(&self) -> WrapperKind {
        self.kind
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub const fn window_width(&self) -> u32 {
        self.window_width
    }

    pub const fn window_height(&self) -> u32 {
        self.window_height
    }

    pub const fn resizable(&self) -> bool {
        self.resizable
    }

    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }

    pub fn storage_namespace(&self) -> &str {
        &self.storage_namespace
    }

    pub const fn release_channel(&self) -> ReleaseChannel {
        self.release_channel
    }

    pub const fn singleton(&self) -> bool {
        self.singleton
    }
}

/// Raw, current-schema product manifest input. It is validated atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductManifestCandidate {
    pub product_id: String,
    pub composition_entrypoints: Vec<String>,
    pub lifecycle: LifecycleMode,
    pub realtime: Option<RealtimeClock>,
    pub kernel_entry: Option<String>,
    /// Current-schema packaged Product Kernel manifest.  This is deliberately
    /// separate from `kernel_entry` so a legacy source-linked module cannot
    /// silently become a Cargo package with a different dependency closure.
    pub kernel_package: Option<String>,
    pub ui_entry: String,
    pub ui_projection_stream: Option<String>,
    pub ui_projection_contract: Option<String>,
    pub content_root: String,
    pub compiled_composition_output: String,
    pub admitted_runtime_content_output: String,
    pub product_assembly_output: String,
    pub product_bundle_output: String,
    pub wrappers: Vec<WrapperCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperCandidate {
    pub id: String,
    pub kind: WrapperKind,
    pub version: String,
    pub application_id: String,
    pub title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub resizable: bool,
    pub permissions: Vec<String>,
    pub storage_namespace: String,
    pub release_channel: ReleaseChannel,
    pub singleton: bool,
}

/// Validated Product Layout. Authored lanes and generated destinations are
/// distinct values so later steps cannot confuse their artifact kinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductManifest {
    product_id: String,
    composition_entrypoints: Vec<ProductPath>,
    lifecycle: LifecycleMode,
    realtime: Option<RealtimeClock>,
    kernel_entry: Option<ProductPath>,
    kernel_package: Option<ProductPath>,
    ui_entry: ProductPath,
    ui_projection_stream: Option<String>,
    ui_projection_contract: Option<String>,
    content_root: ProductPath,
    compiled_composition_output: ProductPath,
    admitted_runtime_content_output: ProductPath,
    product_assembly_output: ProductPath,
    product_bundle_output: ProductPath,
    wrappers: Vec<WrapperDeclaration>,
}

impl ProductManifest {
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    pub fn composition_entrypoints(&self) -> &[ProductPath] {
        &self.composition_entrypoints
    }

    pub const fn lifecycle(&self) -> LifecycleMode {
        self.lifecycle
    }

    pub const fn realtime(&self) -> Option<RealtimeClock> {
        self.realtime
    }

    pub fn kernel_entry(&self) -> Option<&ProductPath> {
        self.kernel_entry.as_ref()
    }

    /// The explicit Cargo manifest for a closed Product Kernel package.
    /// Package admission and closure copying belong to Product Assembly; this
    /// schema owner only validates its bounded product-relative declaration.
    pub fn kernel_package(&self) -> Option<&ProductPath> {
        self.kernel_package.as_ref()
    }

    pub const fn has_kernel(&self) -> bool {
        self.kernel_entry.is_some() || self.kernel_package.is_some()
    }

    pub fn ui_entry(&self) -> &ProductPath {
        &self.ui_entry
    }

    pub fn ui_projection_stream(&self) -> Option<&str> {
        self.ui_projection_stream.as_deref()
    }

    pub fn ui_projection_contract(&self) -> Option<&str> {
        self.ui_projection_contract.as_deref()
    }

    pub fn content_root(&self) -> &ProductPath {
        &self.content_root
    }

    pub fn compiled_composition_output(&self) -> &ProductPath {
        &self.compiled_composition_output
    }

    /// Generated immutable declaration for content admitted by a later runtime
    /// admission step. This crate does not load, admit, or publish it.
    pub fn admitted_runtime_content_output(&self) -> &ProductPath {
        &self.admitted_runtime_content_output
    }

    pub fn product_assembly_output(&self) -> &ProductPath {
        &self.product_assembly_output
    }

    pub fn product_bundle_output(&self) -> &ProductPath {
        &self.product_bundle_output
    }

    pub fn wrappers(&self) -> &[WrapperDeclaration] {
        &self.wrappers
    }
}

/// Decodes and validates a `rusty.toml` manifest without touching the filesystem.
pub fn decode_product_manifest(input: &str) -> Result<ProductManifest, ProductModelError> {
    if input.len() > MAX_PRODUCT_MANIFEST_BYTES {
        return Err(failure(
            "PRODUCT_MANIFEST_BYTES_EXCEEDED",
            SOURCE,
            "$",
            format!("rusty.toml is limited to {MAX_PRODUCT_MANIFEST_BYTES} UTF-8 bytes"),
        ));
    }
    let raw: RawManifest = toml::from_str(input).map_err(|error| {
        failure(
            "PRODUCT_MANIFEST_DECODE",
            SOURCE,
            "$",
            format!("invalid rusty.toml: {error}"),
        )
    })?;
    validate_product_manifest(raw.into_candidate())
}

/// Validates direct manifest input with the same rules as decoded TOML.
pub fn validate_product_manifest(
    candidate: ProductManifestCandidate,
) -> Result<ProductManifest, ProductModelError> {
    validate_identity(&candidate.product_id, SOURCE, "product.id")?;
    validate_realtime(candidate.lifecycle, candidate.realtime)?;
    validate_count(
        candidate.composition_entrypoints.len(),
        1,
        MAX_COMPOSITION_ENTRYPOINTS,
        "runtime_composition.entrypoints",
        "PRODUCT_COMPOSITION_ENTRYPOINT_COUNT",
    )?;

    let mut composition_entrypoints = Vec::with_capacity(candidate.composition_entrypoints.len());
    let mut source_paths = Vec::new();
    for (index, value) in candidate.composition_entrypoints.into_iter().enumerate() {
        let path = ProductPath::parse_at(
            value,
            SOURCE,
            &format!("runtime_composition.entrypoints[{index}]"),
        )?;
        require_lane(
            &path,
            "rules",
            &format!("runtime_composition.entrypoints[{index}]"),
        )?;
        if composition_entrypoints.iter().any(|known| known == &path) {
            return Err(failure(
                "PRODUCT_DUPLICATE_COMPOSITION_ENTRYPOINT",
                SOURCE,
                format!("runtime_composition.entrypoints[{index}]"),
                format!("composition entrypoint `{path}` is declared more than once"),
            ));
        }
        source_paths.push((
            format!("runtime_composition.entrypoints[{index}]"),
            path.clone(),
        ));
        composition_entrypoints.push(path);
    }

    let kernel_entry = candidate
        .kernel_entry
        .map(|value| ProductPath::parse_at(value, SOURCE, "kernel.entry"))
        .transpose()?;
    if let Some(path) = &kernel_entry {
        require_lane(path, "kernel", "kernel.entry")?;
        source_paths.push(("kernel.entry".to_string(), path.clone()));
    }
    let kernel_package = candidate
        .kernel_package
        .map(|value| ProductPath::parse_at(value, SOURCE, "kernel.package"))
        .transpose()?;
    if let Some(path) = &kernel_package {
        require_lane(path, "kernel", "kernel.package")?;
        if !path.as_str().ends_with("/Cargo.toml") {
            return Err(failure(
                "PRODUCT_KERNEL_PACKAGE_MANIFEST",
                SOURCE,
                "kernel.package",
                "kernel.package must name a Cargo.toml manifest inside the kernel lane",
            ));
        }
        source_paths.push(("kernel.package".to_string(), path.clone()));
    }
    if kernel_entry.is_some() && kernel_package.is_some() {
        return Err(failure(
            "PRODUCT_KERNEL_MODE_CONFLICT",
            SOURCE,
            "kernel",
            "declare exactly one of kernel.entry or kernel.package",
        ));
    }

    let ui_entry = ProductPath::parse_at(candidate.ui_entry, SOURCE, "ui.entry")?;
    require_lane(&ui_entry, "ui", "ui.entry")?;
    source_paths.push(("ui.entry".to_string(), ui_entry.clone()));
    if candidate.ui_projection_stream.is_some() != candidate.ui_projection_contract.is_some() {
        return Err(failure(
            "PRODUCT_UI_PROJECTION_PAIR",
            SOURCE,
            "ui",
            "ui.projection_stream and ui.projection_contract must be declared together",
        ));
    }
    if let Some(stream) = &candidate.ui_projection_stream {
        validate_identity(stream, SOURCE, "ui.projection_stream")?;
    }
    if let Some(contract) = &candidate.ui_projection_contract {
        validate_identity(contract, SOURCE, "ui.projection_contract")?;
    }

    let content_root = ProductPath::parse_at(candidate.content_root, SOURCE, "content.root")?;
    if content_root.as_str() != "content" {
        return Err(failure(
            "PRODUCT_CONTENT_LANE",
            SOURCE,
            "content.root",
            "content.root must be exactly the fixed `content` lane",
        ));
    }
    source_paths.push(("content.root".to_string(), content_root.clone()));

    validate_count(
        candidate.wrappers.len(),
        0,
        MAX_WRAPPERS,
        "wrappers",
        "PRODUCT_WRAPPER_COUNT",
    )?;
    let mut wrapper_ids = BTreeSet::new();
    let mut wrappers = Vec::with_capacity(candidate.wrappers.len());
    for (index, wrapper) in candidate.wrappers.into_iter().enumerate() {
        let id_path = format!("wrappers[{index}].id");
        validate_identity(&wrapper.id, SOURCE, &id_path)?;
        if !wrapper_ids.insert(wrapper.id.clone()) {
            return Err(failure(
                "PRODUCT_DUPLICATE_WRAPPER_ID",
                SOURCE,
                id_path,
                format!("wrapper `{}` is declared more than once", wrapper.id),
            ));
        }
        validate_identity(
            &wrapper.application_id,
            SOURCE,
            &format!("wrappers[{index}].application_id"),
        )?;
        validate_semantic_version(&wrapper.version, &format!("wrappers[{index}].version"))?;
        validate_title(&wrapper.title, &format!("wrappers[{index}].title"))?;
        validate_window_dimension(
            wrapper.window_width,
            &format!("wrappers[{index}].window_width"),
        )?;
        validate_window_dimension(
            wrapper.window_height,
            &format!("wrappers[{index}].window_height"),
        )?;
        validate_identity(
            &wrapper.storage_namespace,
            SOURCE,
            &format!("wrappers[{index}].storage_namespace"),
        )?;
        validate_count(
            wrapper.permissions.len(),
            0,
            MAX_WRAPPER_PERMISSIONS,
            &format!("wrappers[{index}].permissions"),
            "PRODUCT_WRAPPER_PERMISSION_COUNT",
        )?;
        let mut permissions = BTreeSet::new();
        for (permission_index, permission) in wrapper.permissions.iter().enumerate() {
            let permission_path = format!("wrappers[{index}].permissions[{permission_index}]");
            validate_identity(permission, SOURCE, &permission_path)?;
            if !permissions.insert(permission.clone()) {
                return Err(failure(
                    "PRODUCT_DUPLICATE_WRAPPER_PERMISSION",
                    SOURCE,
                    permission_path,
                    format!("permission `{permission}` is declared more than once"),
                ));
            }
        }
        wrappers.push(WrapperDeclaration {
            id: wrapper.id,
            kind: wrapper.kind,
            version: wrapper.version,
            application_id: wrapper.application_id,
            title: wrapper.title,
            window_width: wrapper.window_width,
            window_height: wrapper.window_height,
            resizable: wrapper.resizable,
            permissions: wrapper.permissions,
            storage_namespace: wrapper.storage_namespace,
            release_channel: wrapper.release_channel,
            singleton: wrapper.singleton,
        });
    }

    let outputs = [
        (
            "outputs.compiled_composition",
            ProductPath::parse_at(
                candidate.compiled_composition_output,
                SOURCE,
                "outputs.compiled_composition",
            )?,
        ),
        (
            "outputs.admitted_runtime_content",
            ProductPath::parse_at(
                candidate.admitted_runtime_content_output,
                SOURCE,
                "outputs.admitted_runtime_content",
            )?,
        ),
        (
            "outputs.product_assembly",
            ProductPath::parse_at(
                candidate.product_assembly_output,
                SOURCE,
                "outputs.product_assembly",
            )?,
        ),
        (
            "outputs.product_bundle",
            ProductPath::parse_at(
                candidate.product_bundle_output,
                SOURCE,
                "outputs.product_bundle",
            )?,
        ),
    ];
    validate_output_separation(&outputs, &source_paths)?;

    Ok(ProductManifest {
        product_id: candidate.product_id,
        composition_entrypoints,
        lifecycle: candidate.lifecycle,
        realtime: candidate.realtime,
        kernel_entry,
        kernel_package,
        ui_entry,
        ui_projection_stream: candidate.ui_projection_stream,
        ui_projection_contract: candidate.ui_projection_contract,
        content_root,
        compiled_composition_output: outputs[0].1.clone(),
        admitted_runtime_content_output: outputs[1].1.clone(),
        product_assembly_output: outputs[2].1.clone(),
        product_bundle_output: outputs[3].1.clone(),
        wrappers,
    })
}

fn validate_realtime(
    lifecycle: LifecycleMode,
    realtime: Option<RealtimeClock>,
) -> Result<(), ProductModelError> {
    match (lifecycle, realtime) {
        (LifecycleMode::Realtime, Some(clock)) => {
            if !(1..=MAX_REALTIME_HZ).contains(&clock.fixed_step_hz) {
                return Err(failure(
                    "PRODUCT_REALTIME_HZ_BOUNDS",
                    SOURCE,
                    "lifecycle.realtime.fixed_step_hz",
                    format!("fixed_step_hz must be between 1 and {MAX_REALTIME_HZ}"),
                ));
            }
            if !(1..=MAX_REALTIME_CATCH_UP_STEPS).contains(&clock.max_catch_up_steps) {
                return Err(failure(
                    "PRODUCT_REALTIME_CATCH_UP_BOUNDS",
                    SOURCE,
                    "lifecycle.realtime.max_catch_up_steps",
                    format!(
                        "max_catch_up_steps must be between 1 and {MAX_REALTIME_CATCH_UP_STEPS}"
                    ),
                ));
            }
            Ok(())
        }
        (LifecycleMode::Realtime, None) => Err(failure(
            "PRODUCT_REALTIME_SETTINGS_REQUIRED",
            SOURCE,
            "lifecycle.realtime",
            "realtime products require bounded fixed-step settings",
        )),
        (_, Some(_)) => Err(failure(
            "PRODUCT_REALTIME_SETTINGS_INCOMPATIBLE",
            SOURCE,
            "lifecycle.realtime",
            "realtime settings are only valid when lifecycle.mode is `realtime`",
        )),
        (_, None) => Ok(()),
    }
}

fn validate_output_separation(
    outputs: &[(&str, ProductPath); 4],
    sources: &[(String, ProductPath)],
) -> Result<(), ProductModelError> {
    for (output_name, output) in outputs {
        require_lane(output, "generated", output_name)?;
        for (source_name, source) in sources {
            if output.is_within_or_equal(source) || source.is_within_or_equal(output) {
                return Err(failure(
                    "PRODUCT_SOURCE_OUTPUT_OVERLAP",
                    SOURCE,
                    *output_name,
                    format!(
                        "generated output `{output}` overlaps authored source `{source}` at {source_name}"
                    ),
                ));
            }
        }
    }
    for left in 0..outputs.len() {
        for right in (left + 1)..outputs.len() {
            let (left_name, left_path) = &outputs[left];
            let (right_name, right_path) = &outputs[right];
            if left_path.is_within_or_equal(right_path) || right_path.is_within_or_equal(left_path)
            {
                return Err(failure(
                    "PRODUCT_OUTPUT_OVERLAP",
                    SOURCE,
                    *right_name,
                    format!(
                        "generated output `{right_path}` overlaps `{left_path}` at {left_name}"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_count(
    actual: usize,
    minimum: usize,
    maximum: usize,
    path: &str,
    code: &str,
) -> Result<(), ProductModelError> {
    if !(minimum..=maximum).contains(&actual) {
        return Err(failure(
            code,
            SOURCE,
            path,
            format!("must contain between {minimum} and {maximum} entries; found {actual}"),
        ));
    }
    Ok(())
}

fn require_lane(path: &ProductPath, lane: &str, field: &str) -> Result<(), ProductModelError> {
    if !path.starts_in_lane(lane) || path.as_str() == lane {
        return Err(failure(
            "PRODUCT_FIXED_LANE",
            SOURCE,
            field,
            format!("`{path}` must be a descendant of the fixed `{lane}/` lane"),
        ));
    }
    Ok(())
}

fn validate_title(value: &str, path: &str) -> Result<(), ProductModelError> {
    if value.is_empty()
        || value.len() > MAX_WRAPPER_TITLE_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(failure(
            "PRODUCT_INVALID_WRAPPER_TITLE",
            SOURCE,
            path,
            format!(
                "wrapper titles must be trimmed, non-control UTF-8 text up to {MAX_WRAPPER_TITLE_BYTES} bytes"
            ),
        ));
    }
    Ok(())
}

fn validate_window_dimension(value: u32, path: &str) -> Result<(), ProductModelError> {
    if !(MIN_WINDOW_DIMENSION..=MAX_WINDOW_DIMENSION).contains(&value) {
        return Err(failure(
            "PRODUCT_WRAPPER_WINDOW_BOUNDS",
            SOURCE,
            path,
            format!("window dimensions must be between {MIN_WINDOW_DIMENSION} and {MAX_WINDOW_DIMENSION}"),
        ));
    }
    Ok(())
}

fn validate_semantic_version(value: &str, path: &str) -> Result<(), ProductModelError> {
    let invalid = || {
        failure(
            "PRODUCT_INVALID_WRAPPER_VERSION",
            SOURCE,
            path,
            format!(
                "wrapper versions must be strict semantic versions (MAJOR.MINOR.PATCH with optional prerelease/build identifiers) and at most {MAX_WRAPPER_VERSION_BYTES} UTF-8 bytes"
            ),
        )
    };

    if value.is_empty() || value.len() > MAX_WRAPPER_VERSION_BYTES || !value.is_ascii() {
        return Err(invalid());
    }

    let (without_build, build) = match value.split_once('+') {
        Some((core, build)) if !build.is_empty() && !build.contains('+') => (core, Some(build)),
        Some(_) => return Err(invalid()),
        None => (value, None),
    };
    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease)) if !prerelease.is_empty() => (core, Some(prerelease)),
        Some(_) => return Err(invalid()),
        None => (without_build, None),
    };

    let core_parts: Vec<_> = core.split('.').collect();
    if core_parts.len() != 3
        || core_parts
            .iter()
            .any(|part| !valid_numeric_version_identifier(part))
    {
        return Err(invalid());
    }
    if let Some(prerelease) = prerelease {
        if prerelease
            .split('.')
            .any(|part| !valid_prerelease_identifier(part))
        {
            return Err(invalid());
        }
    }
    if let Some(build) = build {
        if build.split('.').any(|part| !valid_build_identifier(part)) {
            return Err(invalid());
        }
    }
    Ok(())
}

fn valid_numeric_version_identifier(value: &str) -> bool {
    !value.is_empty()
        && (value == "0" || !value.starts_with('0'))
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

fn valid_prerelease_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && (!value.bytes().all(|byte| byte.is_ascii_digit())
            || (value == "0" || !value.starts_with('0')))
}

fn valid_build_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

pub(crate) fn validate_identity(
    value: &str,
    source: &str,
    path: &str,
) -> Result<(), ProductModelError> {
    let valid = value.len() <= MAX_IDENTITY_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && !value.as_bytes().windows(2).any(|pair| {
            matches!(pair[0], b'.' | b'_' | b'-') && matches!(pair[1], b'.' | b'_' | b'-')
        });
    if !valid {
        return Err(failure(
            "PRODUCT_INVALID_ID",
            source,
            path,
            format!(
                "identities must be 1..={MAX_IDENTITY_BYTES} lowercase ASCII segments that start and end alphanumeric; dots, underscores, and hyphens may occur singly between segments"
            ),
        ));
    }
    Ok(())
}

/// Validates one runtime caller identity with the same bounded grammar used by
/// current Product Model declarations. This is a pure validation helper; it
/// does not admit a manifest or mutate product state.
pub fn validate_product_identity(value: &str) -> Result<(), ProductModelError> {
    validate_identity(value, SOURCE, "$")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    product: RawProduct,
    runtime_composition: RawRuntimeComposition,
    lifecycle: RawLifecycle,
    kernel: Option<RawKernel>,
    ui: RawUi,
    content: RawContent,
    outputs: RawOutputs,
    #[serde(default)]
    wrappers: Vec<RawWrapper>,
}

impl RawManifest {
    fn into_candidate(self) -> ProductManifestCandidate {
        ProductManifestCandidate {
            product_id: self.product.id,
            composition_entrypoints: self.runtime_composition.entrypoints,
            lifecycle: self.lifecycle.mode,
            realtime: self.lifecycle.realtime,
            kernel_entry: self.kernel.as_ref().and_then(|kernel| kernel.entry.clone()),
            kernel_package: self.kernel.and_then(|kernel| kernel.package),
            ui_entry: self.ui.entry,
            ui_projection_stream: self.ui.projection_stream,
            ui_projection_contract: self.ui.projection_contract,
            content_root: self.content.root,
            compiled_composition_output: self.outputs.compiled_composition,
            admitted_runtime_content_output: self.outputs.admitted_runtime_content,
            product_assembly_output: self.outputs.product_assembly,
            product_bundle_output: self.outputs.product_bundle,
            wrappers: self
                .wrappers
                .into_iter()
                .map(|wrapper| WrapperCandidate {
                    id: wrapper.id,
                    kind: wrapper.kind,
                    version: wrapper.version,
                    application_id: wrapper.application_id,
                    title: wrapper.title,
                    window_width: wrapper.window_width,
                    window_height: wrapper.window_height,
                    resizable: wrapper.resizable,
                    permissions: wrapper.permissions,
                    storage_namespace: wrapper.storage_namespace,
                    release_channel: wrapper.release_channel,
                    singleton: wrapper.singleton,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProduct {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRuntimeComposition {
    entrypoints: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLifecycle {
    mode: LifecycleMode,
    realtime: Option<RealtimeClock>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKernel {
    entry: Option<String>,
    package: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawUi {
    entry: String,
    projection_stream: Option<String>,
    projection_contract: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContent {
    root: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutputs {
    compiled_composition: String,
    admitted_runtime_content: String,
    product_assembly: String,
    product_bundle: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWrapper {
    id: String,
    kind: WrapperKind,
    version: String,
    application_id: String,
    title: String,
    window_width: u32,
    window_height: u32,
    resizable: bool,
    permissions: Vec<String>,
    storage_namespace: String,
    release_channel: ReleaseChannel,
    singleton: bool,
}
