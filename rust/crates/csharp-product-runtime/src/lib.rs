//! Deliberately permissive loader for one trusted C# product.
//!
//! This is the Engine's direct product runtime, not a hostile plugin boundary or
//! compatibility protocol. The product is first-party trusted code. This adapter
//! owns the fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its application state and orchestration.

pub use csharp_engine_abi::*;

use std::{
    ffi::c_void,
    fs,
    path::{Path, PathBuf},
    ptr,
    sync::Arc,
    time::Instant,
};

use csharp_engine_services::{
    CsharpAppearanceCallOutput, CsharpAppearanceCatalog, CsharpEngineCallOutput,
    CsharpEngineServicesError, EngineServiceSet,
};
use libloading::Library;
use netcorehost::{
    hostfxr::{HostfxrContext, InitializedForRuntimeConfig},
    nethost,
    pdcstring::PdCString,
};
pub use product_container::ProductSource;
use product_host::{
    runtime_fault_disposition, CanonicalU64, ProductHostControlOperation, ProductHostDebugResult,
    ProductHostFaultDisposition, ProductHostInputBatch, ProductHostInputResult,
    ProductHostLifecycleOperation, ProductHostLog, ProductHostLogDisposition, ProductHostLogEvent,
    ProductHostLogSeverity, ProductHostOperationKind, ProductHostOperationResult,
    ProductHostRendererStatus, ProductHostRendererWidget, ProductHostRuntime,
    ProductHostRuntimeBinding, ProductHostRuntimeError, ProductHostRuntimeFault,
    ProductHostRuntimeReadout, ProductHostRuntimeReceipt, ProductHostRuntimeScheduleState,
    ProductHostRuntimeState, ProductHostTimelineCompletion, ProductHostTimelineCompletionResult,
    ProductHostUpdateAttribution,
};
use product_host::{RuntimePublication, RuntimePublicationError, RuntimePublicationFrontier};
use runtime_input::{
    self as runtime_input_model, AxisValue, CompiledInputMappings, DirectInputIntentDescriptor,
    InputAxis, InputClearReason, InputContext, InputEdge, IntentValueKind,
    RuntimeDirectIntentClaim, RuntimeInputBinding, RuntimeInputEvent, RuntimeInputFact,
    RuntimeInputIngress, RuntimeInputLane, RuntimeInputMapping, RuntimeInputTrigger,
    RuntimeIntentEnvelope, RuntimeIntentValue,
};
use runtime_lifecycle::{
    ExternalStep, HostMonotonicTime, RealtimeLifecycleConfig, RuntimeControlOperation,
    RuntimeInstanceId, RuntimeLifecycle, RuntimeLifecycleConfig, RuntimeLifecycleReadout,
    RuntimeMode, RuntimeState,
};
use runtime_ui::RuntimeUiRuntimeBinding;

const ABI_OK: i32 = 1;
const STANDARD_REALTIME_HZ: u32 = 60;
const STANDARD_MAX_CATCH_UP_STEPS: u32 = 4;
const STANDARD_REALTIME_EXERCISE_ADMISSION_NS: u64 = 16_666_667;
/// Native input events are held only until the next admitted product update.
/// One accepted wire batch plus the Engine-inserted rebind clear. Additional
/// cadence pressure is resynchronized rather than silently truncating history.
const MAX_PENDING_NATIVE_INPUTS: usize = runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS + 1;

#[derive(Debug, PartialEq, Eq)]
enum PendingInputAdmission {
    Appended,
    Resynchronized,
}
// The Engine-owned Product Browser Host uses this same default when its
// generated bundle does not supply an input-context override. Keeping the
// standard native runtime on that typed host default lets generated physical
// input reach RuntimeInputLane without product-local bundle edits.
const STANDARD_INPUT_CONTEXT: &str = "gameplay.default";
const REALTIME_UPDATE_MODE: NativeProductUpdateMode = NativeProductUpdateMode::Realtime;
const DEMAND_UPDATE_MODE: NativeProductUpdateMode = NativeProductUpdateMode::Demand;
const EXTERNAL_UPDATE_MODE: NativeProductUpdateMode = NativeProductUpdateMode::External;
// These are host admission bounds, before the immutable Content service owns
// references. The per-file limit matches the Engine renderer resource limit;
// the aggregate limit matches the existing product persistence payload limit.
const MAX_PRODUCT_ABI_IDENTITY_BYTES: usize = 128;
const HOST_ABI_BUILD_IDENTITY: &[u8] = b"rusty-engine-host/v1";

type CoreclrProductBindV1 = unsafe extern "system" fn(
    *const NativeProductAbiHandshakeV1,
    *mut NativeProductAbiHandshakeV1,
) -> i32;

/// Exact ABI/runtime identity published by the generic packaged product host.
///
/// This is diagnostic provenance for a matched runtime pack, not a negotiated
/// compatibility surface. Product construction still requires the V1 handshake
/// to match exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductHostRuntimeIdentity {
    pub protocol_version: u32,
    pub engine_api_bytes: usize,
    pub product_api_bytes: usize,
    pub fingerprint: NativeProductAbiFingerprint,
    pub build_identity: &'static str,
}

impl ProductHostRuntimeIdentity {
    pub fn fingerprint_hex(self) -> String {
        abi_fingerprint_text(self.fingerprint)
    }
}

pub fn product_host_runtime_identity() -> ProductHostRuntimeIdentity {
    ProductHostRuntimeIdentity {
        protocol_version: PRODUCT_ABI_PROTOCOL_VERSION,
        engine_api_bytes: std::mem::size_of::<NativeEngineApi>(),
        product_api_bytes: std::mem::size_of::<NativeProductApi>(),
        fingerprint: PRODUCT_ABI_FINGERPRINT,
        build_identity: std::str::from_utf8(HOST_ABI_BUILD_IDENTITY)
            .expect("fixed UTF-8 Engine host identity"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RendererDebugCommand {
    Presentation,
    Read,
    Show,
    Hide,
    Toggle,
    Status,
}

/// `engine.renderer.snapshot <path>`: the one renderer command with an argument.
const SCENE_SNAPSHOT_COMMAND: &str = "engine.renderer.snapshot";

/// Keeps the Engine's renderer-debug vocabulary exact. Product-owned command
/// prefixes continue through the generated callback untouched.
fn renderer_debug_command(command: &str) -> Option<RendererDebugCommand> {
    match command.trim() {
        "engine.renderer" => Some(RendererDebugCommand::Read),
        "engine.renderer.presentation" => Some(RendererDebugCommand::Presentation),
        "engine.renderer.show" => Some(RendererDebugCommand::Show),
        "engine.renderer.hide" => Some(RendererDebugCommand::Hide),
        "engine.renderer.toggle" => Some(RendererDebugCommand::Toggle),
        "engine.renderer.status" => Some(RendererDebugCommand::Status),
        _ => None,
    }
}

fn pretty_json(value: &impl serde::Serialize) -> Result<String, ProductHostRuntimeError> {
    serde_json::to_string_pretty(value).map_err(|error| {
        ProductHostRuntimeError::new(
            "CSHARP_RENDERER_DIAGNOSTICS_ENCODE",
            format!("renderer answer could not be encoded: {error}"),
        )
    })
}

#[derive(Clone, Debug)]
pub struct CsharpProductRuntimeError {
    code: &'static str,
    detail: String,
}

/// Where committed audio plays: the manifest's `audio.output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOutputSelection {
    /// This process's default output device. `required` fails the load
    /// without one; otherwise the product runs silent.
    Device { required: bool },
    /// Mixed in real time and streamed to the pages watching the frames.
    Stream,
}

/// Explicit standard-runtime configuration. Lifecycle selection, direct input
/// descriptors, and physical mappings are Engine-owned host configuration,
/// not product policy.
#[derive(Clone)]
pub struct CsharpProductRuntimeConfig {
    /// The host/supervisor-owned identity for this loaded runtime incarnation.
    /// It is deliberately separate from the lifecycle generation and control
    /// revision, which both begin afresh inside each incarnation.
    runtime_instance_id: RuntimeInstanceId,
    lifecycle: RuntimeLifecycleConfig,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    physical_mappings: Vec<RuntimeInputMapping>,
    input_cursor_mode: NativeInputCursorMode,
    /// Optional host-selected application root for opaque product state.
    /// Products choose only relative scopes beneath this root.
    persistence_root: Option<PathBuf>,
    diagnostics: ProductHostLog,
    renderer_options: render_wgpu::RendererOptions,
    /// Where the runtime's renderer draws; `None` builds no renderer.
    render_output: Option<RenderOutput>,
    /// Where committed audio plays.
    audio_output: AudioOutputSelection,
    /// The desktop shell's device, for window output.
    window_gpu: Option<render_wgpu::Gpu>,
    /// The Product a scene snapshot names.
    product: Option<scene_snapshot::SceneSnapshotProduct>,
}

impl CsharpProductRuntimeConfig {
    pub fn new(
        runtime_instance_id: RuntimeInstanceId,
        lifecycle: RuntimeLifecycleConfig,
        direct_intents: Vec<DirectInputIntentDescriptor>,
    ) -> Self {
        Self {
            runtime_instance_id,
            lifecycle,
            direct_intents,
            physical_mappings: Vec::new(),
            input_cursor_mode: NativeInputCursorMode::PointerLock,
            persistence_root: None,
            diagnostics: ProductHostLog::new(Default::default())
                .expect("fixed diagnostic defaults"),
            renderer_options: render_wgpu::RendererOptions::default(),
            render_output: None,
            audio_output: AudioOutputSelection::Device { required: false },
            window_gpu: None,
            product: None,
        }
    }

    /// The Product's manifest identity, which a scene snapshot records.
    pub fn with_product(mut self, id: impl Into<String>, title: impl Into<String>) -> Self {
        self.product = Some(scene_snapshot::SceneSnapshotProduct {
            id: id.into(),
            title: title.into(),
        });
        self
    }

    /// Renders the world in this process, to `output`, and plays its audio
    /// on the output device. The product host always selects one.
    pub fn with_render_output(mut self, output: RenderOutput) -> Self {
        self.render_output = Some(output);
        self
    }

    /// The manifest's `audio.output`: the watching pages, or this process's
    /// device and whether a missing one fails the load.
    pub fn with_audio_output(mut self, output: AudioOutputSelection) -> Self {
        self.audio_output = output;
        self
    }

    /// The desktop shell's device: in window output the runtime's renderer
    /// is built on it and the shell draws it.
    pub fn with_window_gpu(mut self, gpu: render_wgpu::Gpu) -> Self {
        self.window_gpu = Some(gpu);
        self
    }

    /// The product manifest's default light rigs, for a renderer in this
    /// process.
    pub fn with_default_lights(mut self, world: bool, viewmodel: bool) -> Self {
        self.renderer_options.default_world_lights = world;
        self.renderer_options.default_viewmodel_lights = viewmodel;
        self
    }

    /// The product manifest's renderer settings: the renderer's initial
    /// values, which `RendererSettings` changes at runtime.
    pub fn with_renderer_settings(
        mut self,
        settings: &render_model::RendererSettingsDescriptor,
    ) -> Self {
        self.renderer_options = self.renderer_options.with_settings(settings);
        self
    }

    /// Adds typed standard-runtime physical mappings to create-time host
    /// configuration. The product receives a copied descriptor; it does not
    /// own or mutate the runtime lane's mapping evaluation.
    pub fn with_physical_mappings(mut self, mappings: Vec<RuntimeInputMapping>) -> Self {
        self.physical_mappings = mappings;
        self
    }

    /// Selects the Engine-owned browser cursor behavior for gameplay input.
    /// Pointer lock remains the default for existing first-person products.
    pub fn with_input_cursor_mode(mut self, cursor_mode: NativeInputCursorMode) -> Self {
        self.input_cursor_mode = cursor_mode;
        self
    }

    /// Selects the explicit host-owned root used by product persistence.
    /// There is no implicit filesystem root; omitting this option leaves the
    /// persistence service unconfigured for products that do not need it.
    pub fn with_persistence_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.persistence_root = Some(root.into());
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: ProductHostLog) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}

impl CsharpProductRuntimeError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub const fn code(&self) -> &'static str {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for CsharpProductRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for CsharpProductRuntimeError {}

impl From<CsharpEngineServicesError> for CsharpProductRuntimeError {
    fn from(error: CsharpEngineServicesError) -> Self {
        Self::new(error.code(), error.detail().to_owned())
    }
}

impl From<CsharpProductRuntimeError> for ProductHostRuntimeError {
    fn from(error: CsharpProductRuntimeError) -> Self {
        ProductHostRuntimeError::new(error.code, error.detail)
    }
}

enum LoadedProductHost {
    // NativeAOT initializes process-wide managed runtime support. It does not
    // provide a safe shared-library unload contract, so a successfully created
    // product keeps its library mapped until process exit after destroy.
    NativeAot(Option<Library>),
    // The hostfxr context owns the initialized CoreCLR lifetime. Its managed
    // function pointers remain callable until the product has been destroyed.
    CoreClr { _host: CoreclrProductHost },
}

struct CoreclrProductHost {
    _context: HostfxrContext<InitializedForRuntimeConfig>,
}

struct LoadedProductApi {
    host: LoadedProductHost,
    create: NativeProductCreate,
    start: NativeProductAction,
    update: NativeProductUpdate,
    complete_timeline: NativeProductCompleteTimeline,
    paused_intents: NativeProductPausedIntents,
    pause: NativeProductAction,
    resume: NativeProductAction,
    restart: NativeProductAction,
    shutdown: NativeProductAction,
    destroy: NativeProductDestroy,
    execute_debug: NativeProductExecuteDebug,
    describe_debug: NativeProductDescribeDebug,
    release_debug_result: NativeProductReleaseDebugResult,
    observe_runtime: NativeProductObserveRuntime,
    read_call_error: NativeProductReadCallError,
    release_call_error: NativeProductReleaseCallError,
}

impl LoadedProductApi {
    fn load_nativeaot(path: &Path) -> Result<Self, CsharpProductRuntimeError> {
        // SAFETY: Loading is the explicitly requested trusted-first-party
        // product boundary. `Library` remains owned by `Self` until after the
        // product instance has been destroyed in `CsharpProductRuntime::drop`.
        let library = unsafe { Library::new(path) }.map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_LIBRARY_LOAD",
                format!("{}: {error}", path.display()),
            )
        })?;
        // SAFETY: every function pointer is copied from a required fixed symbol
        // while the owning `Library` is retained in this struct.
        unsafe fn symbol<T: Copy>(
            library: &Library,
            name: &[u8],
        ) -> Result<T, CsharpProductRuntimeError> {
            // SAFETY: the API deliberately fixes the expected C ABI signatures;
            // a mismatched trusted product is outside this experiment's safety
            // contract and is rejected when an expected symbol is absent.
            unsafe { library.get::<T>(name) }
                .map(|value| *value)
                .map_err(|error| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_REQUIRED_EXPORT",
                        format!(
                            "required NativeAOT export `{}` is unavailable: {error}",
                            String::from_utf8_lossy(&name[..name.len() - 1])
                        ),
                    )
                })
        }
        let bind: NativeProductBindV1 = unsafe { symbol(&library, b"rusty_product_bind_v1\0") }?;
        Self::from_bound_product(
            product_from_bind(bind)?,
            LoadedProductHost::NativeAot(Some(library)),
        )
    }

    fn load_coreclr(
        assembly_path: &Path,
        runtime_config_path: &Path,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let assembly = coreclr_path(
            assembly_path,
            "CSHARP_CORECLR_ASSEMBLY",
            "managed product assembly",
        )?;
        let runtime_config = coreclr_path(
            runtime_config_path,
            "CSHARP_CORECLR_RUNTIMECONFIG",
            "managed product runtimeconfig",
        )?;
        let assembly_name = assembly_path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_CORECLR_ASSEMBLY",
                    format!(
                        "managed product assembly `{}` needs a UTF-8 file stem",
                        assembly_path.display()
                    ),
                )
            })?;
        let type_label = format!("Rusty.Engine.NativeProduct.ProductExports, {assembly_name}");
        let type_name = PdCString::from_os_str(&type_label).map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_CORECLR_BIND_EXPORT",
                format!("could not encode generated product export type: {error}"),
            )
        })?;
        let method_name = PdCString::from_os_str("BindV1").expect("fixed managed method name");
        let hostfxr = nethost::load_hostfxr().map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_CORECLR_HOSTFXR",
                format!("could not locate/load hostfxr: {error}"),
            )
        })?;
        let context = hostfxr
            .initialize_for_runtime_config(&runtime_config)
            .map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_CORECLR_RUNTIMECONFIG",
                    format!(
                        "could not initialize CoreCLR from `{}`: {error}",
                        runtime_config_path.display()
                    ),
                )
            })?;
        let loader = context
            .get_delegate_loader_for_assembly(assembly)
            .map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_CORECLR_BIND_EXPORT",
                    format!(
                        "could not prepare generated product export from `{}`: {error}",
                        assembly_path.display()
                    ),
                )
            })?;
        // `ProductExports.BindV1` is generated with UnmanagedCallersOnly and
        // `CallConvCdecl`; on the supported x64 hosts `system` is the native
        // ABI used by hostfxr's typed delegate loader.
        let bind = loader
            .get_function_with_unmanaged_callers_only::<CoreclrProductBindV1>(
                &type_name,
                &method_name,
            )
            .map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_CORECLR_BIND_EXPORT",
                    format!("generated export `{type_label}.BindV1` is unavailable: {error}",),
                )
            })?;
        Self::from_bound_product(
            product_from_coreclr_bind(*bind)?,
            LoadedProductHost::CoreClr {
                _host: CoreclrProductHost { _context: context },
            },
        )
    }

    fn from_bound_product(
        product: NativeProductApi,
        host: LoadedProductHost,
    ) -> Result<Self, CsharpProductRuntimeError> {
        Ok(Self {
            create: required_function(product.create, "create")?,
            start: required_function(product.start, "start")?,
            update: required_function(product.update, "update")?,
            complete_timeline: required_function(product.complete_timeline, "complete_timeline")?,
            paused_intents: required_function(product.paused_intents, "paused_intents")?,
            pause: required_function(product.pause, "pause")?,
            resume: required_function(product.resume, "resume")?,
            restart: required_function(product.restart, "restart")?,
            shutdown: required_function(product.shutdown, "shutdown")?,
            destroy: required_function(product.destroy, "destroy")?,
            execute_debug: required_function(product.execute_debug, "execute_debug")?,
            describe_debug: required_function(product.describe_debug, "describe_debug")?,
            release_debug_result: required_function(
                product.release_debug_result,
                "release_debug_result",
            )?,
            observe_runtime: required_function(product.observe_runtime, "observe_runtime")?,
            read_call_error: required_function(product.read_call_error, "read_call_error")?,
            release_call_error: required_function(
                product.release_call_error,
                "release_call_error",
            )?,
            host,
        })
    }
}

fn product_from_bind(
    bind: NativeProductBindV1,
) -> Result<NativeProductApi, CsharpProductRuntimeError> {
    let host = host_abi_handshake();
    let mut product = NativeProductAbiHandshakeV1::default();
    // SAFETY: descriptor storage has exact C layout. The product receives no
    // host-owned API table to write and returns only its immutable table pointer.
    let status = unsafe { bind(&host, &mut product) };
    checked_status(status, "bind")?;
    product_from_handshake(&host, product)
}

fn product_from_coreclr_bind(
    bind: CoreclrProductBindV1,
) -> Result<NativeProductApi, CsharpProductRuntimeError> {
    let host = host_abi_handshake();
    let mut product = NativeProductAbiHandshakeV1::default();
    // SAFETY: descriptor storage has exact C layout and `bind` is the
    // hostfxr-resolved generated V1 entry point. No product API storage is
    // passed to the product; it returns an owned immutable table pointer.
    let status = unsafe { bind(&host, &mut product) };
    checked_status(status, "bind")?;
    product_from_handshake(&host, product)
}

fn host_abi_handshake() -> NativeProductAbiHandshakeV1 {
    NativeProductAbiHandshakeV1 {
        protocol_version: PRODUCT_ABI_PROTOCOL_VERSION,
        engine_api_size: std::mem::size_of::<NativeEngineApi>(),
        product_api_size: std::mem::size_of::<NativeProductApi>(),
        fingerprint: PRODUCT_ABI_FINGERPRINT,
        build_identity: NativeUtf8Slice {
            bytes: HOST_ABI_BUILD_IDENTITY.as_ptr(),
            len: HOST_ABI_BUILD_IDENTITY.len(),
        },
        product_api: ptr::null(),
    }
}

fn product_from_handshake(
    host: &NativeProductAbiHandshakeV1,
    product: NativeProductAbiHandshakeV1,
) -> Result<NativeProductApi, CsharpProductRuntimeError> {
    if host.protocol_version != product.protocol_version
        || host.engine_api_size != product.engine_api_size
        || host.product_api_size != product.product_api_size
        || host.fingerprint != product.fingerprint
    {
        return Err(abi_handshake_mismatch(host, &product));
    }
    if product.product_api.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_ABI_POINTER",
            "V1 ABI handshake matched but the product returned a null immutable product table pointer",
        ));
    }
    // SAFETY: exact V1 protocol/table-size/fingerprint equality was checked
    // before copying the product-owned immutable table. A non-null pointer is
    // the remaining fixed ABI pointer/length coherence requirement.
    Ok(unsafe { *product.product_api })
}

fn abi_handshake_mismatch(
    host: &NativeProductAbiHandshakeV1,
    product: &NativeProductAbiHandshakeV1,
) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(
        "CSHARP_PRODUCT_ABI_MISMATCH",
        format!(
            "expected protocol={} engine_table_bytes={} product_table_bytes={} fingerprint={} host_build={}; observed protocol={} engine_table_bytes={} product_table_bytes={} fingerprint={} sdk_build={}; restore the matching runtime pack or rebuild the product",
            host.protocol_version,
            host.engine_api_size,
            host.product_api_size,
            abi_fingerprint_text(host.fingerprint),
            abi_identity_text(host.build_identity),
            product.protocol_version,
            product.engine_api_size,
            product.product_api_size,
            abi_fingerprint_text(product.fingerprint),
            abi_identity_text(product.build_identity),
        ),
    )
}

fn abi_fingerprint_text(value: NativeProductAbiFingerprint) -> String {
    format!(
        "{:016x}{:016x}{:016x}{:016x}",
        value.word0, value.word1, value.word2, value.word3
    )
}

fn abi_identity_text(value: NativeUtf8Slice) -> String {
    if value.len == 0 {
        return "<missing>".to_owned();
    }
    if value.len > MAX_PRODUCT_ABI_IDENTITY_BYTES || value.bytes.is_null() {
        return "<invalid>".to_owned();
    }
    // SAFETY: the V1 bind contract keeps the bounded identity valid until the
    // host has copied it during this immediate handshake result processing.
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(value.bytes, value.len) })
        .into_owned()
}

fn coreclr_path(
    path: &Path,
    code: &'static str,
    label: &str,
) -> Result<PdCString, CsharpProductRuntimeError> {
    if !path.is_file() {
        return Err(CsharpProductRuntimeError::new(
            code,
            format!("{label} `{}` is not a file", path.display()),
        ));
    }
    PdCString::from_os_str(path.as_os_str()).map_err(|error| {
        CsharpProductRuntimeError::new(
            code,
            format!(
                "{label} `{}` has an unsupported path: {error}",
                path.display()
            ),
        )
    })
}

fn required_function<T>(function: Option<T>, name: &str) -> Result<T, CsharpProductRuntimeError> {
    function.ok_or_else(|| {
        CsharpProductRuntimeError::new(
            "CSHARP_REQUIRED_FUNCTION",
            format!("C# product did not bind required function `{name}`"),
        )
    })
}

mod audio_output;
mod frame_output;
mod render_output;
pub mod scene_snapshot;

pub use product_host::ProductHostRenderOutput as RenderOutput;

pub use frame_output::{WindowFrame, WindowTiming};
pub use render_wgpu::{Gpu, Renderer, SceneDriver};

mod playtest;

/// A finished product call: what it published, any input mapping it
/// selected, and the product exception or Engine error that ended it.
struct FinishedProductCall {
    outputs: Vec<RuntimePublication>,
    input_mapping_replacement: Option<runtime_input::CompiledInputMappings>,
    failure: Option<CsharpProductRuntimeError>,
}

/// The fixed-step rate of a runtime that has a gameplay rate.
fn gameplay_cadence(lifecycle: &RuntimeLifecycle) -> Option<u32> {
    match lifecycle.configuration() {
        RuntimeLifecycleConfig::Realtime(config) => Some(config.fixed_step_hz()),
        RuntimeLifecycleConfig::Demand | RuntimeLifecycleConfig::External => None,
    }
}

fn apply_gameplay_time(
    lifecycle: &mut RuntimeLifecycle,
    request: csharp_engine_services::GameplayTimeRequest,
) -> Result<runtime_lifecycle::GameplayTime, runtime_lifecycle::RuntimeLifecycleError> {
    match request {
        csharp_engine_services::GameplayTimeRequest::Rate(rate) => {
            lifecycle.select_gameplay_rate(rate)
        }
        csharp_engine_services::GameplayTimeRequest::Advance { steps, rate } => {
            lifecycle.begin_gameplay_advance(steps, rate)
        }
    }
}

pub struct CsharpProductRuntime {
    playtest_time: playtest::TimeMode,
    api: LoadedProductApi,
    handle: *mut c_void,
    lifecycle: RuntimeLifecycle,
    input_lane: RuntimeInputLane,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    pending_inputs: Vec<NativeInputOwned>,
    /// A recovery baseline produced while input was being admitted. The host
    /// can call `input` without receiving an operation receipt, so retain at
    /// most the latest bounded baseline until the next scheduled receipt can
    /// publish it.
    pending_recovery_outputs: Vec<RuntimePublication>,
    services: Box<EngineServiceSet>,
    /// The Product and content root whose bundle inventory `reload_content`
    /// re-admits.
    content_source: ProductSource,
    content_root: String,
    initial_output: Option<Vec<RuntimePublication>>,
    renderer_metrics_visible: bool,
    shutdown_called: bool,
    diagnostics: ProductHostLog,
    pending_update_attribution: Option<ProductHostUpdateAttribution>,
    /// The gameplay time the last product call selected, settled onto the
    /// lifecycle once that call's own transition (if any) has applied.
    staged_gameplay_time: Option<csharp_engine_services::GameplayTimeRequest>,
    /// Unscaled host time observed since the last realtime update, which the
    /// next one reports as `host_elapsed_seconds`.
    host_elapsed_ns: u64,
    /// Present when audio plays on this process's output device.
    audio_output: Option<audio_output::AudioOutput>,
    /// The renderer, when the configuration selected an output: absent only
    /// in runtimes built without one (tests).
    frame_output: Option<frame_output::FrameOutput>,
    /// How the page presents the product: its surface, UI scale and
    /// anchored rects. The renderer follows the anchors as they arrive.
    presentation: Arc<product_host::ProductHostPresentation>,
    /// Runs the product's RenderOutput jobs.
    render_outputs: render_output::OutputExecutor,
    /// What a scene snapshot records about the renderer and the Product.
    renderer_options: render_wgpu::RendererOptions,
    product: Option<scene_snapshot::SceneSnapshotProduct>,
    /// A harness holding input (`control/claim`), until it releases or its
    /// lease passes without input.
    input_claim: Option<InputClaim>,
}

struct InputClaim {
    label: String,
    lease: std::time::Duration,
    renewed: std::time::Instant,
}

/// The longest input lease a harness may hold between inputs.
const MAX_INPUT_CLAIM_LEASE: std::time::Duration = std::time::Duration::from_secs(3600);

// The product host serializes every call with one mutex. The native handle
// has no ambient access from Rust and is destroyed before the retained product
// host is released (or the NativeAOT mapping is retained for process exit).
unsafe impl Send for CsharpProductRuntime {}

/// Callback state remains Engine-owned for the complete loaded-product lifetime.
/// A C# call only borrows its value arena; Rust copies it into envelopes and commits
impl CsharpProductRuntime {
    /// Loads one NativeAOT C# library and creates its authoritative product state.
    pub fn load(
        library_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let content = CsharpProductContent::admit(content_root)?;
        Self::load_admitted(library_path, content, config)
    }

    /// Loads one NativeAOT product from content already read and admitted before host startup.
    pub fn load_admitted(
        library_path: impl AsRef<Path>,
        content: CsharpProductContent,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        Self::load_admitted_with(content, config, || {
            LoadedProductApi::load_nativeaot(library_path.as_ref())
        })
    }

    /// Loads one ordinary managed C# product through the development-only
    /// CoreCLR hostfxr path. The product still binds the generated native table
    /// and executes against the same Engine services as NativeAOT.
    pub fn load_coreclr(
        assembly_path: impl AsRef<Path>,
        runtime_config_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let content = CsharpProductContent::admit(content_root)?;
        Self::load_coreclr_admitted(assembly_path, runtime_config_path, content, config)
    }

    /// Loads a managed CoreCLR product from content already read and admitted
    /// before host startup.
    pub fn load_coreclr_admitted(
        assembly_path: impl AsRef<Path>,
        runtime_config_path: impl AsRef<Path>,
        content: CsharpProductContent,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        Self::load_admitted_with(content, config, || {
            LoadedProductApi::load_coreclr(assembly_path.as_ref(), runtime_config_path.as_ref())
        })
    }

    fn load_admitted_with(
        content: CsharpProductContent,
        config: CsharpProductRuntimeConfig,
        load_api: impl FnOnce() -> Result<LoadedProductApi, CsharpProductRuntimeError>,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let persistence_root = prepare_persistence_root(config.persistence_root.as_deref())?;
        let frame_output = config
            .render_output
            .map(|output| {
                frame_output::FrameOutput::start(
                    output,
                    config.renderer_options,
                    config.window_gpu.as_ref(),
                )
            })
            .transpose()?;
        let presentation = product_host::ProductHostPresentation::new();
        if let Some(frames) = &frame_output {
            let driver = frames.driver();
            presentation.set_anchor_listener(move |anchors| {
                driver.set_viewport_anchors(anchors.clone());
            });
        }
        // The process that draws the world plays its sound.
        let audio_output = match frame_output {
            Some(_) => audio_output::AudioOutput::open(config.audio_output)?,
            None => None,
        };
        let render_outputs = render_output::OutputExecutor::start(config.renderer_options)
            .map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_RENDER_OUTPUT",
                    format!("could not start the render output worker: {error}"),
                )
            })?;
        let mut input_mappings = CompiledInputMappings::standard(
            config.direct_intents.clone(),
            config.physical_mappings.clone(),
        )
        .map_err(input_error)?;
        let lifecycle = RuntimeLifecycle::new(config.runtime_instance_id, config.lifecycle);
        let initial_binding = input_binding(&lifecycle);
        let input_context = standard_input_context().as_str().as_bytes().to_vec();
        let native_input_descriptors: Vec<NativeInputDescriptor> = config
            .direct_intents
            .iter()
            .map(|descriptor| {
                let payload_contract = descriptor
                    .payload_contract()
                    .map_or(ptr::null(), |value| value.as_bytes().as_ptr());
                let payload_contract_len = descriptor.payload_contract().map_or(0, str::len);
                NativeInputDescriptor {
                    id: descriptor.id().as_bytes().as_ptr(),
                    id_len: descriptor.id().len(),
                    value_kind: native_input_value_kind(descriptor.value_kind()),
                    payload_contract,
                    payload_contract_len,
                }
            })
            .collect();
        let mut native_mapping_chords = Vec::with_capacity(config.physical_mappings.len());
        let native_physical_mappings: Vec<NativeInputMapping> = config
            .physical_mappings
            .iter()
            .map(|mapping| native_input_mapping(mapping, &mut native_mapping_chords))
            .collect();
        let native_input = NativeInputConfiguration {
            binding: NativeInputBinding {
                instance_id: initial_binding.instance_id().value(),
                generation: initial_binding.generation().value(),
                control_revision: initial_binding.control_revision().value(),
            },
            cursor_mode: config.input_cursor_mode,
            context: input_context.as_ptr(),
            context_len: input_context.len(),
            direct_intents: native_input_descriptors.as_ptr(),
            direct_intents_len: native_input_descriptors.len(),
            physical_mappings: native_physical_mappings.as_ptr(),
            physical_mappings_len: native_physical_mappings.len(),
        };
        let api = load_api()?;
        let CsharpProductContent {
            source: content_source,
            root: content_root,
            files: content,
            appearance_catalog,
            bundles,
        } = content;
        let content_resources = content
            .iter()
            .map(|file| {
                let path = std::str::from_utf8(&file.path)
                    .expect("collected product paths are UTF-8")
                    .to_owned();
                (path, Arc::clone(&file.bytes))
            })
            .collect();
        // The generated ABI stores raw context pointers. Boxing the whole
        // service set keeps every callback context at one stable address for
        // the complete lifetime of the product.
        let mut services = Box::new(EngineServiceSet::with_direct_intents(
            appearance_catalog,
            content_resources,
            persistence_root,
            config.diagnostics.handle(),
            config.direct_intents.clone(),
        )?);
        services.bind_content_bundles(bundles);
        if let Some(frames) = &frame_output {
            services.ingest_renderer_settings(frames.settings_readout());
        }
        let native_content: Vec<NativeContentFile> = content
            .iter()
            .map(|file| NativeContentFile {
                path: file.path.as_ptr(),
                path_len: file.path.len(),
                bytes: file.bytes.as_ptr(),
                bytes_len: file.bytes.len(),
            })
            .collect();
        let args = NativeProductCreateArgs {
            content: native_content.as_ptr(),
            content_len: native_content.len(),
            input: native_input,
            engine: services.api(),
        };
        let mut handle = ptr::null_mut();
        services.set_gameplay_time(gameplay_cadence(&lifecycle), lifecycle.gameplay_time());
        services.begin_create_call(ui_binding(&lifecycle));
        let created = call_create(&api, &args, &mut handle).and_then(|()| {
            if handle.is_null() {
                return Err(CsharpProductRuntimeError::new(
                    "CSHARP_CREATE_HANDLE",
                    "rusty_product_create succeeded but returned a null product handle",
                ));
            }
            let mut call = services.finish_call()?;
            // Convert once and retain the owned create output for the first Start.
            let initial_output = call_outputs(&render_outputs, call.take_output())?;
            services.seal_resource_selection();
            Ok((
                initial_output,
                call.take_input_mapping_replacement(),
                services.take_gameplay_time_request(),
            ))
        });
        let (initial_output, initial_input_mapping_replacement, initial_gameplay_time) =
            match created {
                Ok(created) => created,
                Err(error) => {
                    let _ = services.finish_call();
                    if !handle.is_null() {
                        // SAFETY: a failing create may still have returned an owned
                        // handle; releasing it is part of the fixed ownership ABI.
                        unsafe { (api.destroy)(handle) };
                    }
                    return Err(error);
                }
            };
        let mut initial_output = initial_output;
        if audio_output.is_some() {
            // Retained voices from create reach the device in the Start
            // baseline; the device does not play before Start.
            audio_output::take_audio_ops(&mut initial_output);
        }
        if let Some(frames) = &frame_output {
            // The renderer shows the created world before Start publishes it.
            frames.realize(
                &services,
                &initial_output,
                frame_output::Simulation {
                    held: true,
                    step: lifecycle.readout().admitted_simulation_steps(),
                },
            );
        }
        let initial_output = Some(initial_output);
        observe_product_runtime(&api, handle, lifecycle.readout());
        if let Some(replacement) = initial_input_mapping_replacement {
            input_mappings = replacement;
        }
        let input_lane =
            RuntimeInputLane::new(input_mappings, initial_binding, standard_input_context());
        Ok(Self {
            playtest_time: playtest::TimeMode::Realtime,
            api,
            handle,
            lifecycle,
            input_lane,
            direct_intents: config.direct_intents,
            pending_inputs: Vec::new(),
            pending_recovery_outputs: Vec::new(),
            services,
            content_source,
            content_root,
            initial_output,
            renderer_metrics_visible: false,
            shutdown_called: false,
            diagnostics: config.diagnostics,
            pending_update_attribution: None,
            staged_gameplay_time: initial_gameplay_time,
            host_elapsed_ns: 0,
            audio_output,
            frame_output,
            presentation,
            render_outputs,
            renderer_options: config.renderer_options,
            product: config.product,
            input_claim: None,
        })
    }

    /// The one standard realtime host configuration. Demand and external modes
    /// have no Engine timing policy and use their respective lifecycle variants.
    pub fn standard_realtime_config() -> RuntimeLifecycleConfig {
        RuntimeLifecycleConfig::Realtime(
            RealtimeLifecycleConfig::new(STANDARD_REALTIME_HZ, STANDARD_MAX_CATCH_UP_STEPS)
                .expect("fixed standard realtime configuration"),
        )
    }

    /// Runs provider-fixture assertions, not a general product health check.
    /// Requires fixture UI, voxel, input, timeline and fault/restart behavior;
    /// see docs/csharp-product-project.md#host-exercise-contract.
    /// Exercises the selected lifecycle mode plus its rejected neighbouring
    /// operation. Rejection happens before the NativeAOT product update, so its
    /// pending input and lifecycle counters remain unchanged.
    pub fn exercise_updates(&mut self) -> Result<(), CsharpProductRuntimeError> {
        self.start_for_exercise()?;
        self.exercise_fresh_attachments()?;
        let started_binding = input_binding(&self.lifecycle);
        self.exercise_ui_projection_binding(started_binding)?;
        self.input(ProductHostInputBatch::new(vec![key_press(
            started_binding,
            1,
        )]))
        .map_err(exercise_runtime_error)?;
        self.control(
            ProductHostControlOperation::Replace,
            dev_binding_from_input(started_binding),
        )
        .map_err(exercise_runtime_error)?;
        let replaced_binding = input_binding(&self.lifecycle);
        if replaced_binding.generation() != started_binding.generation()
            || replaced_binding.control_revision() == started_binding.control_revision()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_REPLACE",
                "control replacement changed simulation identity or did not advance revision",
            ));
        }
        if self.pending_inputs.len() != 1
            || self.pending_inputs[0].kind != NativeInputEventKind::Clear
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_CLEAR",
                "control replacement did not retain only its clear input",
            ));
        }
        if self
            .input(ProductHostInputBatch::new(vec![input_clear(
                started_binding,
                2,
            )]))
            .is_ok_and(|receipt| receipt.result().is_accepted())
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_STALE_INPUT",
                "stale input was admitted after a control revision",
            ));
        }
        self.exercise_ui_projection_binding(replaced_binding)?;
        self.input(ProductHostInputBatch::new(vec![key_press(
            replaced_binding,
            1,
        )]))
        .map_err(exercise_runtime_error)?;
        let binding_after_product_input = input_binding(&self.lifecycle);
        if binding_after_product_input != replaced_binding {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_PRODUCT_INPUT",
                "ordinary product input replaced the control binding",
            ));
        }
        self.control(
            ProductHostControlOperation::Release,
            dev_binding_from_input(replaced_binding),
        )
        .map_err(exercise_runtime_error)?;
        let released_binding = input_binding(&self.lifecycle);
        if released_binding.generation() != replaced_binding.generation()
            || released_binding.control_revision() == replaced_binding.control_revision()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_RELEASE",
                "control release changed simulation identity or did not advance revision",
            ));
        }
        if self
            .control(
                ProductHostControlOperation::Release,
                dev_binding_from_input(replaced_binding),
            )
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_STALE_CONTROL",
                "stale control release was admitted after a control revision",
            ));
        }
        self.input(ProductHostInputBatch::new(vec![input_clear(
            released_binding,
            1,
        )]))
        .map_err(exercise_runtime_error)?;
        self.exercise_physical_mapping(released_binding)?;
        self.exercise_direct_intent(released_binding)?;
        self.exercise_selected_mode()?;
        self.exercise_timeline_completion()?;
        self.exercise_pause_resume()?;
        Ok(())
    }

    /// Measures the ordinary demand-update path through lifecycle admission,
    /// the generated C# callback, Engine service staging/commit, and output
    /// conversion. This is an explicit diagnostic probe, never runtime policy.
    pub fn performance_probe_demand(
        &mut self,
        iterations: u32,
    ) -> Result<Vec<u128>, CsharpProductRuntimeError> {
        if iterations == 0 || iterations > 256 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_PERFORMANCE_ITERATIONS",
                "performance probe iterations must be in 1..=256",
            ));
        }
        if self.lifecycle.mode() != RuntimeMode::Demand {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_PERFORMANCE_MODE",
                "performance probe requires demand lifecycle mode",
            ));
        }
        if self.lifecycle.state() == RuntimeState::Created {
            ProductHostRuntime::lifecycle(self, ProductHostLifecycleOperation::Start).map_err(
                |error| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_PERFORMANCE_RUNTIME",
                        format!("{}: {}", error.code(), error.diagnostic()),
                    )
                },
            )?;
        }
        for _ in 0..iterations.min(8) {
            ProductHostRuntime::admit_demand_step(self).map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_PERFORMANCE_RUNTIME",
                    format!("{}: {}", error.code(), error.diagnostic()),
                )
            })?;
        }
        let mut durations = Vec::with_capacity(iterations as usize);
        for _ in 0..iterations {
            let started = Instant::now();
            ProductHostRuntime::admit_demand_step(self).map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_PERFORMANCE_RUNTIME",
                    format!("{}: {}", error.code(), error.diagnostic()),
                )
            })?;
            durations.push(started.elapsed().as_nanos());
        }
        Ok(durations)
    }

    fn exercise_fresh_attachments(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let before = self.readout();
        let (_, first_outputs) = self.connect().map_err(exercise_runtime_error)?.into_parts();
        assert_ui_projection_binding(&first_outputs, input_binding(&self.lifecycle))?;
        let first_voxel = complete_voxel_baseline(&first_outputs)?;

        let (_, second_outputs) = self.connect().map_err(exercise_runtime_error)?.into_parts();
        assert_ui_projection_binding(&second_outputs, input_binding(&self.lifecycle))?;
        let second_voxel = complete_voxel_baseline(&second_outputs)?;
        if second_voxel != first_voxel || self.readout() != before {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_ATTACH",
                "repeated browser attachment changed active runtime state or voxel baseline identity",
            ));
        }
        Ok(())
    }

    fn exercise_timeline_completion(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let binding = self.binding();
        let completion = ProductHostTimelineCompletion::decode_json(
            &serde_json::to_vec(&serde_json::json!({
                "ticket": "7",
                "runtime": {
                    "instanceId": binding.instance_id.get().to_string(),
                    "generation": binding.generation.get().to_string(),
                    "controlRevision": binding.control_revision.get().to_string(),
                },
                "correlation": "runtime.exercise.timeline",
                "outcome": {
                    "kind": "success",
                    "data": { "accepted": true },
                },
                "provenance": {
                    "correlation": "runtime.exercise.timeline",
                    "detail": { "source": "fixture" },
                },
            }))
            .map_err(|error| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_TIMELINE",
                    format!("timeline fixture encoding failed: {error}"),
                )
            })?,
        )
        .map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_TIMELINE",
                format!("timeline fixture admission failed: {error}"),
            )
        })?;
        let receipt = self
            .complete_timeline(completion)
            .map_err(exercise_runtime_error)?;
        if !receipt.result().is_accepted() || receipt.result().ticket().get() != 7 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_TIMELINE",
                "C# product did not accept the copied timeline completion",
            ));
        }
        Ok(())
    }

    fn exercise_ui_projection_binding(
        &mut self,
        expected: RuntimeInputBinding,
    ) -> Result<(), CsharpProductRuntimeError> {
        let receipt = match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                let baseline = self
                    .lifecycle
                    .readout()
                    .last_observed_time()
                    .map(|value| value.nanoseconds())
                    .unwrap_or(0);
                self.advance_realtime(CanonicalU64::new(baseline))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(
                    baseline
                        .checked_add(STANDARD_REALTIME_EXERCISE_ADMISSION_NS)
                        .ok_or_else(|| {
                            CsharpProductRuntimeError::new(
                                "CSHARP_EXERCISE_UI_BINDING",
                                "realtime UI projection observation overflowed",
                            )
                        })?,
                ))
                .map_err(exercise_runtime_error)?
            }
            RuntimeMode::Demand => self.admit_demand_step().map_err(exercise_runtime_error)?,
            RuntimeMode::External => self
                .admit_external_step(CanonicalU64::new(
                    self.lifecycle.readout().admitted_simulation_steps(),
                ))
                .map_err(exercise_runtime_error)?,
        };
        let (_, outputs) = receipt.into_parts();
        assert_ui_projection_binding(&outputs, expected).map(|_| ())
    }

    fn exercise_pause_resume(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let running_binding = self.binding();
        let (_, paused_outputs) = self
            .lifecycle_with_binding(ProductHostLifecycleOperation::Pause, Some(running_binding))
            .map_err(exercise_runtime_error)?
            .into_parts();
        if self.lifecycle.state() != RuntimeState::Paused {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_PAUSE",
                "pause did not leave the Rust lifecycle paused",
            ));
        }
        // The fixture's Pause publishes nothing; the current projection still
        // follows the paused binding, so the browser does not blank its UI.
        assert_ui_projection_binding(&paused_outputs, input_binding(&self.lifecycle))?;
        let paused_binding = self.binding();
        let paused_operation = self.advance_realtime(CanonicalU64::new(0));
        if paused_operation.is_ok() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_PAUSE",
                "a paused lifecycle admitted realtime work",
            ));
        }
        self.lifecycle_with_binding(ProductHostLifecycleOperation::Resume, Some(paused_binding))
            .map_err(exercise_runtime_error)?;
        if self.lifecycle.state() != RuntimeState::Running {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_RESUME",
                "resume did not leave the Rust lifecycle running",
            ));
        }
        match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                self.advance_realtime(CanonicalU64::new(0))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(STANDARD_REALTIME_EXERCISE_ADMISSION_NS))
                    .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                let step = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(step))
                    .map_err(exercise_runtime_error)?;
            }
        }
        self.exercise_fault_restart()?;
        Ok(())
    }

    fn exercise_fault_restart(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let before_fault = self.lifecycle.readout();
        let before_binding = input_binding(&self.lifecycle);
        let fault_sequence = self
            .input_lane
            .last_sequence()
            .and_then(|sequence| sequence.checked_add(1))
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_FAULT",
                    "fault input sequence overflowed",
                )
            })?;
        self.input(ProductHostInputBatch::new(vec![fault_key_press(
            before_binding,
            fault_sequence,
        )]))
        .map_err(exercise_runtime_error)?;
        match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                let baseline = self
                    .lifecycle
                    .readout()
                    .last_observed_time()
                    .map(|value| value.nanoseconds())
                    .unwrap_or(0);
                self.advance_realtime(CanonicalU64::new(
                    baseline
                        .checked_add(STANDARD_REALTIME_EXERCISE_ADMISSION_NS)
                        .ok_or_else(|| {
                            CsharpProductRuntimeError::new(
                                "CSHARP_EXERCISE_FAULT",
                                "fault exercise observation overflowed",
                            )
                        })?,
                ))
                .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                let step = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(step))
                    .map_err(exercise_runtime_error)?;
            }
        }
        let faulted = self.lifecycle.readout();
        if faulted.state() != RuntimeState::Faulted
            || faulted.fault() != Some(runtime_lifecycle::RuntimeFault::OwnerReported)
            || faulted.generation() != before_fault.generation()
            || faulted.admitted_simulation_steps()
                != before_fault
                    .admitted_simulation_steps()
                    .checked_add(1)
                    .ok_or_else(|| {
                        CsharpProductRuntimeError::new(
                            "CSHARP_EXERCISE_FAULT",
                            "fault exercise simulation counter overflowed",
                        )
                    })?
            || faulted.admitted_presentations() != before_fault.admitted_presentations()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_FAULT",
                "product fault result did not preserve the completed update counters and typed fault state",
            ));
        }
        if self
            .input(ProductHostInputBatch::new(vec![key_press(
                before_binding,
                1,
            )]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_FAULT",
                "faulted lifecycle admitted input from its pre-fault binding",
            ));
        }

        let fault_binding = input_binding(&self.lifecycle);
        let restart = self
            .lifecycle_with_binding(
                ProductHostLifecycleOperation::Restart,
                Some(dev_binding_from_input(fault_binding)),
            )
            .map_err(exercise_runtime_error)?;
        let (_, outputs) = restart.into_parts();
        assert_ui_projection_binding(&outputs, input_binding(&self.lifecycle))?;
        let restarted = self.lifecycle.readout();
        if restarted.state() != RuntimeState::Running
            || restarted.generation().value() != before_fault.generation().value() + 1
            || restarted.admitted_simulation_steps() != 0
            || restarted.admitted_presentations() != 0
            || restarted.fault().is_some()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_RESTART",
                "restart did not create a fresh running generation",
            ));
        }
        if self
            .input(ProductHostInputBatch::new(vec![key_press(
                before_binding,
                1,
            )]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_RESTART",
                "pre-restart input binding remained admitted after restart",
            ));
        }
        match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                self.advance_realtime(CanonicalU64::new(0))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(STANDARD_REALTIME_EXERCISE_ADMISSION_NS))
                    .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                self.admit_external_step(CanonicalU64::new(0))
                    .map_err(exercise_runtime_error)?;
            }
        }
        if self.lifecycle.readout().admitted_simulation_steps() == 0 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_RESTART",
                "fresh restarted generation did not admit a product update",
            ));
        }
        Ok(())
    }

    fn exercise_physical_mapping(
        &mut self,
        current_binding: RuntimeInputBinding,
    ) -> Result<(), CsharpProductRuntimeError> {
        self.input(ProductHostInputBatch::new(vec![key_press(
            current_binding,
            2,
        )]))
        .map_err(exercise_runtime_error)?;
        let baseline = self
            .lifecycle
            .readout()
            .last_observed_time()
            .map(|value| value.nanoseconds())
            .unwrap_or(0);
        let realtime_observation = |multiplier: u64| {
            baseline
                .checked_add(STANDARD_REALTIME_EXERCISE_ADMISSION_NS * multiplier)
                .ok_or_else(|| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_EXERCISE_REALTIME",
                        "realtime exercise observation overflowed",
                    )
                })
        };
        match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                self.advance_realtime(CanonicalU64::new(baseline))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(realtime_observation(1)?))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(realtime_observation(2)?))
                    .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                let first = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(first))
                    .map_err(exercise_runtime_error)?;
                let second = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(second))
                    .map_err(exercise_runtime_error)?;
            }
        }
        self.input(ProductHostInputBatch::new(vec![key_release(
            current_binding,
            3,
        )]))
        .map_err(exercise_runtime_error)?;
        match self.lifecycle.mode() {
            RuntimeMode::Realtime => {
                self.advance_realtime(CanonicalU64::new(realtime_observation(3)?))
                    .map_err(exercise_runtime_error)?;
                self.advance_realtime(CanonicalU64::new(realtime_observation(4)?))
                    .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                let first = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(first))
                    .map_err(exercise_runtime_error)?;
                let second = self.lifecycle.readout().admitted_simulation_steps();
                self.admit_external_step(CanonicalU64::new(second))
                    .map_err(exercise_runtime_error)?;
            }
        }
        Ok(())
    }

    fn exercise_direct_intent(
        &mut self,
        current_binding: RuntimeInputBinding,
    ) -> Result<(), CsharpProductRuntimeError> {
        let descriptor = self
            .direct_intents
            .iter()
            .find(|candidate| candidate.value_kind() == IntentValueKind::ProductPayload)
            .cloned()
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_INTENT_CONFIG",
                    "payload exercise requires one configured payload direct intent",
                )
            })?;
        let stale_revision = current_binding
            .control_revision()
            .value()
            .checked_sub(1)
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_STALE_DIRECT_INTENT",
                    "direct-intent exercise requires a prior control revision",
                )
            })?;
        let stale_binding = RuntimeInputBinding::new(
            current_binding.instance_id(),
            current_binding.generation(),
            runtime_lifecycle::RuntimeControlRevision::new(stale_revision),
        );
        let next_sequence = self
            .input_lane
            .last_sequence()
            .and_then(|sequence| sequence.checked_add(1))
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_DIRECT_INTENT",
                    "direct-intent exercise sequence overflowed",
                )
            })?;
        let stale = direct_intent(stale_binding, next_sequence, &descriptor)?;
        if self
            .input(ProductHostInputBatch::new(vec![stale]))
            .is_ok_and(|receipt| receipt.result().is_accepted())
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_STALE_DIRECT_INTENT",
                "stale direct intent was admitted after a control rebind",
            ));
        }
        let contract = descriptor.payload_contract().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_DIRECT_INTENT",
                "configured payload direct intent has no payload contract",
            )
        })?;
        let unmapped =
            payload_intent(current_binding, next_sequence, "runtime.unmapped", contract)?;
        if self
            .input(ProductHostInputBatch::new(vec![unmapped]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_UNMAPPED_DIRECT_INTENT",
                "unmapped payload direct intent was admitted",
            ));
        }
        let mismatched = payload_intent(
            current_binding,
            next_sequence,
            descriptor.id(),
            "runtime.wrong.contract",
        )?;
        if self
            .input(ProductHostInputBatch::new(vec![mismatched]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_MISMATCHED_DIRECT_INTENT",
                "payload direct intent with a mismatched contract was admitted",
            ));
        }
        let admitted = direct_intent(current_binding, next_sequence, &descriptor)?;
        self.input(ProductHostInputBatch::new(vec![admitted]))
            .map_err(exercise_runtime_error)?;
        // Direct claims deliberately remain in RuntimeInputLane until the
        // next admitted input snapshot. The generated fixture checks their
        // copied ProductInputEvent shape in that callback.
        let digital = self
            .direct_intents
            .iter()
            .find(|candidate| candidate.value_kind() == IntentValueKind::Digital)
            .cloned()
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_INTENT_CONFIG",
                    "digital exercise requires one configured direct intent",
                )
            })?;
        let admitted = direct_intent(
            current_binding,
            next_sequence.checked_add(1).ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_DIRECT_INTENT",
                    "direct-intent exercise sequence overflowed",
                )
            })?,
            &digital,
        )?;
        self.input(ProductHostInputBatch::new(vec![admitted]))
            .map_err(exercise_runtime_error)?;
        Ok(())
    }

    fn exercise_selected_mode(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let selected_mode = self.lifecycle.mode();
        let readout_mode = self.readout().mode();
        let expected_readout_mode = match selected_mode {
            RuntimeMode::Realtime => product_host::ProductHostRuntimeMode::Realtime,
            RuntimeMode::Demand => product_host::ProductHostRuntimeMode::Demand,
            RuntimeMode::External => product_host::ProductHostRuntimeMode::External,
        };
        if readout_mode != expected_readout_mode {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_MODE_READOUT",
                "runtime readout did not report the selected lifecycle mode",
            ));
        }
        let admitted_before = self.lifecycle.readout().admitted_simulation_steps();
        let pending_before = self.pending_inputs.len();
        let rejected = match selected_mode {
            RuntimeMode::Realtime => self.admit_demand_step().is_err(),
            RuntimeMode::Demand => self.advance_realtime(CanonicalU64::new(0)).is_err(),
            RuntimeMode::External => self.admit_demand_step().is_err(),
        };
        if !rejected
            || self.lifecycle.readout().admitted_simulation_steps() != admitted_before
            || self.pending_inputs.len() != pending_before
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_WRONG_MODE",
                "wrong lifecycle mode reached Product.Game or changed lifecycle admission",
            ));
        }

        let expected_admission_increment = 1;
        match selected_mode {
            RuntimeMode::Realtime => {
                let baseline = self
                    .lifecycle
                    .readout()
                    .last_observed_time()
                    .map(|value| value.nanoseconds())
                    .unwrap_or(0);
                self.advance_realtime(CanonicalU64::new(baseline))
                    .map_err(exercise_runtime_error)?;
                let observation = CanonicalU64::new(
                    baseline
                        .checked_add(STANDARD_REALTIME_EXERCISE_ADMISSION_NS)
                        .ok_or_else(|| {
                            CsharpProductRuntimeError::new(
                                "CSHARP_EXERCISE_REALTIME",
                                "realtime exercise observation overflowed",
                            )
                        })?,
                );
                self.advance_realtime(observation)
                    .map_err(exercise_runtime_error)?;
            }
            RuntimeMode::Demand => {
                self.admit_demand_step().map_err(exercise_runtime_error)?;
            }
            RuntimeMode::External => {
                let accepted_step = CanonicalU64::new(admitted_before);
                self.admit_external_step(accepted_step)
                    .map_err(exercise_runtime_error)?;
                let admitted_after = self.lifecycle.readout().admitted_simulation_steps();
                let pending_after = self.pending_inputs.len();
                let skipped_step =
                    CanonicalU64::new(admitted_after.checked_add(2).ok_or_else(|| {
                        CsharpProductRuntimeError::new(
                            "CSHARP_EXERCISE_EXTERNAL_STEP",
                            "external exercise step identity overflowed",
                        )
                    })?);
                if self.admit_external_step(accepted_step).is_ok()
                    || self.admit_external_step(skipped_step).is_ok()
                    || self.lifecycle.readout().admitted_simulation_steps() != admitted_after
                    || self.pending_inputs.len() != pending_after
                {
                    return Err(CsharpProductRuntimeError::new(
                        "CSHARP_EXERCISE_EXTERNAL_STEP",
                        "duplicate or skipped external steps reached Product.Game or lifecycle admission",
                    ));
                }
            }
        };
        if self.lifecycle.readout().admitted_simulation_steps()
            != admitted_before
                .checked_add(expected_admission_increment)
                .expect("successful lifecycle admission cannot overflow")
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_ADMISSION",
                "selected lifecycle mode did not admit exactly one product update",
            ));
        }
        Ok(())
    }

    fn update(
        &mut self,
        facts: NativeProductUpdateFacts,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        if let Some(audio) = &mut self.audio_output {
            audio.report(&mut self.services)?;
        }
        if let Some(frames) = &mut self.frame_output {
            frames.report(&mut self.services);
        }
        let watching = self
            .frame_output
            .as_ref()
            .is_none_or(frame_output::FrameOutput::watching);
        let (surface, anchors) = match self.presentation.layout() {
            Some(layout) => (
                NativeCameraSurfaceReadout {
                    reported: true,
                    watching,
                    css_width: layout.css_width,
                    css_height: layout.css_height,
                    device_width: layout.css_width * layout.device_pixel_ratio,
                    device_height: layout.css_height * layout.device_pixel_ratio,
                    device_pixel_ratio: layout.device_pixel_ratio,
                    ui_scale: layout.ui_scale,
                    revision: layout.revision,
                },
                layout.anchors,
            ),
            None => (
                NativeCameraSurfaceReadout {
                    watching,
                    ..Default::default()
                },
                Default::default(),
            ),
        };
        self.services.ingest_camera_surface(surface, anchors);
        self.services
            .begin_update_call(ui_binding(&self.lifecycle), facts);
        let callback_started = Instant::now();
        let callback_result = call_update(
            &self.api,
            self.handle,
            NativeProductUpdateArgs {
                facts,
                events: events.as_ptr(),
                event_count: events.len(),
            },
        );
        let callback_duration_us = callback_started
            .elapsed()
            .as_micros()
            .min(u128::from(u64::MAX)) as u64;
        self.pending_update_attribution = Some(ProductHostUpdateAttribution::from(
            self.services
                .complete_update_attribution(callback_duration_us),
        ));
        let post_callback_started = Instant::now();
        let update_binding = self.binding();
        if let Some(attribution) = &mut self.pending_update_attribution {
            attribution.runtime = Some(update_binding);
            attribution.simulation_step = CanonicalU64::new(facts.simulation_step);
            attribution.admitted_step_count =
                CanonicalU64::new(u64::from(facts.admitted_step_count));
        }
        // The callback consumed its input whatever happened inside it.
        self.pending_inputs.clear();
        let finished = self.finish_product_call(callback_result.as_ref().err().cloned());
        self.settle_gameplay_time();
        if !events.is_empty() {
            let step = self.lifecycle.readout().admitted_simulation_steps();
            if let Some(frames) = &mut self.frame_output {
                frames.record_input_step(step);
            }
        }
        let mut outputs = finished.outputs;
        let input_mapping_replacement = finished.input_mapping_replacement;
        if let Some(failure) = finished.failure {
            self.fault_after_call(
                "update",
                Some(&failure),
                input_mapping_replacement,
                &mut outputs,
            )?;
        } else if matches!(callback_result, Ok(NativeProductUpdateResult::ReportFault)) {
            self.fault_after_call("update", None, input_mapping_replacement, &mut outputs)?;
        } else if let Some(replacement) = input_mapping_replacement {
            self.settle_input_mapping_replacement(replacement)?;
            outputs = self.rebind_outputs_in_place(outputs)?;
        }
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        if let Some(attribution) = &mut self.pending_update_attribution {
            attribution.post_callback_duration_us = CanonicalU64::new(
                post_callback_started
                    .elapsed()
                    .as_micros()
                    .min(u128::from(u64::MAX)) as u64,
            );
        }
        Ok(outputs)
    }

    fn update_admitted(
        &mut self,
        kind: NativeProductUpdateMode,
        observed_host_time_nanoseconds: Option<u64>,
        admission: runtime_lifecycle::SimulationAdmission,
        dropped_step_count: u128,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        // Realtime catch-up remains one product update per host
        // observation. Correlate that update with the last lifecycle-admitted
        // step while Runtime Input retains all ingress and held state once.
        let step = admission
            .step_at(admission.step_count().saturating_sub(1))
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_INPUT_ADMISSION",
                    "lifecycle admission did not expose its admitted step",
                )
            })?;
        let (_, envelopes) = self
            .input_lane
            .snapshot_for_step(&self.lifecycle, step.phases().input_snapshot())
            .map_err(input_error)?;
        let context = self.input_lane.context().clone();
        let mapped = envelopes
            .iter()
            .map(|envelope| native_intent_event(envelope, &context))
            .collect::<Vec<_>>();
        if self.append_pending_inputs(mapped)? == PendingInputAdmission::Resynchronized {
            return Ok(Vec::new());
        }
        let host_elapsed = self.take_host_elapsed(observed_host_time_nanoseconds);
        let facts = update_facts(
            &self.lifecycle,
            kind,
            observed_host_time_nanoseconds,
            (admission.first_step().value(), admission.step_count()),
            dropped_step_count,
            host_elapsed,
        )?;
        self.update(facts)
    }

    /// The host time a realtime update reports; an update without a host
    /// observation (an inspection step) reports none and leaves it.
    fn take_host_elapsed(&mut self, observed_host_time_nanoseconds: Option<u64>) -> f64 {
        if observed_host_time_nanoseconds.is_none() {
            return 0.0;
        }
        std::mem::take(&mut self.host_elapsed_ns) as f64 / 1e9
    }

    /// The update a product that selected gameplay time receives for a host
    /// observation that admitted no step: its input and a chance to present
    /// and choose, with no world time.
    fn update_without_step(
        &mut self,
        observed_host_time_nanoseconds: u64,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        // The same input owner and snapshot as a step: edges and deltas are
        // consumed once, here, and held state is reported again.
        let (_, envelopes) = self
            .input_lane
            .snapshot_without_step(&self.lifecycle)
            .map_err(input_error)?;
        let context = self.input_lane.context().clone();
        let mapped = envelopes
            .iter()
            .map(|envelope| native_intent_event(envelope, &context))
            .collect::<Vec<_>>();
        if self.append_pending_inputs(mapped)? == PendingInputAdmission::Resynchronized {
            return Ok(Vec::new());
        }
        let next_step = self.lifecycle.readout().admitted_simulation_steps();
        let host_elapsed = self.take_host_elapsed(Some(observed_host_time_nanoseconds));
        let facts = update_facts(
            &self.lifecycle,
            REALTIME_UPDATE_MODE,
            Some(observed_host_time_nanoseconds),
            (next_step, 0),
            0,
            host_elapsed,
        )?;
        self.update(facts)
    }

    /// Hands the direct UI claims a paused runtime admitted to the product in
    /// one call. Simulation stays paused: no step, time or Update is
    /// admitted. A product failure faults the runtime as any callback does;
    /// the second value says whether it did.
    fn deliver_paused_intents(
        &mut self,
        intents: Vec<RuntimeIntentEnvelope>,
    ) -> Result<(Vec<RuntimePublication>, bool), CsharpProductRuntimeError> {
        if intents.is_empty() {
            return Ok((Vec::new(), false));
        }
        let context = self.input_lane.context().clone();
        let owned = intents
            .iter()
            .map(|envelope| native_intent_event(envelope, &context))
            .collect::<Vec<_>>();
        let events = owned
            .iter()
            .map(NativeInputOwned::as_native)
            .collect::<Vec<_>>();
        self.services.begin_call(ui_binding(&self.lifecycle));
        let callback_result = call_paused_intents(&self.api, self.handle, &events);
        let finished = self.finish_product_call(callback_result.err());
        self.settle_gameplay_time();
        let mut outputs = finished.outputs;
        let Some(failure) = finished.failure else {
            return Ok((outputs, false));
        };
        self.fault_after_call(
            "paused_intents",
            Some(&failure),
            finished.input_mapping_replacement,
            &mut outputs,
        )?;
        Ok((outputs, true))
    }

    /// Finishes the product call. Engine services keep everything the call
    /// did; nothing is rolled back. The failure is the product's exception,
    /// or an Engine error while settling the call's renderer work.
    fn finish_product_call(
        &mut self,
        callback_error: Option<CsharpProductRuntimeError>,
    ) -> FinishedProductCall {
        let mut finished = FinishedProductCall {
            outputs: Vec::new(),
            input_mapping_replacement: None,
            failure: callback_error,
        };
        let call = self.services.finish_call();
        // Kept even when finishing failed: the product's request succeeded.
        if let Some(request) = self.services.take_gameplay_time_request() {
            self.staged_gameplay_time = Some(request);
        }
        match call {
            Ok(mut call) => {
                finished.input_mapping_replacement = call.take_input_mapping_replacement();
                match call_outputs(&self.render_outputs, call.take_output()) {
                    Ok(mut outputs) => {
                        if let Some(audio) = &mut self.audio_output {
                            audio.realize(&self.services, &mut outputs);
                        }
                        if let Some(frames) = &self.frame_output {
                            frames.realize(&self.services, &outputs, self.frame_simulation());
                        }
                        outputs.retain(|output| !is_empty_frame(output));
                        finished.outputs = outputs;
                    }
                    Err(error) => {
                        finished.failure.get_or_insert(error);
                        self.rebaseline_frames();
                    }
                }
            }
            Err(error) => {
                finished.failure.get_or_insert(error.into());
                self.rebaseline_frames();
            }
        }
        self.follow_simulation_with_frames();
        finished
    }

    /// Applies the gameplay time the last call selected to the lifecycle and
    /// tells the service what is now in force. It applies from the next host
    /// observation: steps the current update already admitted are delivered.
    fn settle_gameplay_time(&mut self) {
        if let Some(request) = self.staged_gameplay_time.take() {
            if let Err(error) = apply_gameplay_time(&mut self.lifecycle, request) {
                let error = self.lifecycle_runtime_error(error);
                self.publish_diagnostic(&error);
            }
        }
        self.services.set_gameplay_time(
            gameplay_cadence(&self.lifecycle),
            self.lifecycle.gameplay_time(),
        );
        self.follow_world_time();
    }

    /// Stops simulation after a product exception, an Engine failure while
    /// finishing its call, or a product-reported fault. The product stays
    /// loaded for inspection; Resume continues it and Restart resets it.
    /// Renderers get a fresh baseline, since a failure may have lost some of
    /// the call's renderer work.
    fn fault_after_call(
        &mut self,
        operation: &str,
        failure: Option<&CsharpProductRuntimeError>,
        input_mapping_replacement: Option<runtime_input::CompiledInputMappings>,
        outputs: &mut Vec<RuntimePublication>,
    ) -> Result<(), CsharpProductRuntimeError> {
        if let Some(failure) = failure {
            // The diagnostics log keeps the full record; the process stream
            // names the fault so it is visible without one.
            eprintln!("{}", fault_line(operation, failure));
            let _ = self.diagnostics.publish(
                ProductHostLogEvent::new(
                    ProductHostLogSeverity::Error,
                    ProductHostLogDisposition::Degraded,
                    "csharp-runtime",
                    failure.code(),
                    format!(
                        "{}\nSimulation is paused with the product loaded; resume or restart to continue.",
                        failure.detail()
                    ),
                )
                .expect("a runtime error code is a bounded identity")
                .with_runtime(self.binding()),
            );
        }
        if let Some(replacement) = input_mapping_replacement {
            self.input_lane
                .replace_physical_mappings(replacement)
                .map_err(input_error)?;
        }
        if matches!(
            self.lifecycle.state(),
            RuntimeState::Running | RuntimeState::Paused
        ) {
            self.lifecycle
                .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
                .map_err(lifecycle_error)?;
            self.rebind_input(InputClearReason::ControlRevisionChange)?;
        }
        let binding = self.binding();
        // The browser clears UI when the binding changes, so each stream's
        // latest projection follows the fault binding, as in an in-place rebind.
        outputs.retain(|output| !matches!(output, RuntimePublication::UiProjection(_)));
        outputs.push(self.binding_output());
        outputs.extend(
            self.services
                .snapshot_ui_projections(ui_binding(&self.lifecycle))
                .into_iter()
                .map(RuntimePublication::UiProjection),
        );
        outputs.push(self.complete_baseline_output(binding)?);
        Ok(())
    }

    /// Advances the input binding fence after a host or callback-facing queue
    /// overflow. The dropped prefix cannot be replayed safely: RuntimeInputLane
    /// has already admitted its sequence, while the product callback has not
    /// observed it. Rebinding clears held/pending state and gives the next
    /// update an explicit clear event on a fresh control revision.
    fn recover_pending_input_overflow(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.lifecycle
            .change_control(RuntimeControlOperation::Replace)
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        // RuntimeInputLane's control-fence contract uses this reason for any
        // same-generation rebind; the overflow code remains explicit in the
        // surrounding runtime diagnostic.
        self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
            .map_err(|error| self.runtime_error(error))?;
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        // A previous admission failure may already have staged a recovery
        // baseline. Replace it with this newer fence rather than accumulating
        // receipts when input pressure persists across host observations.
        self.pending_recovery_outputs.clear();
        self.receipt(
            ProductHostOperationKind::ReplaceControl,
            self.rebind_in_place(Vec::new())?,
        )
    }

    /// Appends callback-facing native input only when the bounded queue can
    /// retain the complete ordered prefix. A partial append would create a
    /// sequence hole at the C# boundary, so overflow clears and rebinds the
    /// entire input lane instead.
    fn append_pending_inputs(
        &mut self,
        inputs: Vec<NativeInputOwned>,
    ) -> Result<PendingInputAdmission, CsharpProductRuntimeError> {
        if self
            .pending_inputs
            .len()
            .checked_add(inputs.len())
            .is_none_or(|length| length > MAX_PENDING_NATIVE_INPUTS)
        {
            let error = CsharpProductRuntimeError::new(
                "CSHARP_INPUT_PENDING_BOUNDS",
                format!(
                    "pending native input exceeded the {}-event callback bound; input was resynchronized",
                    MAX_PENDING_NATIVE_INPUTS
                ),
            );
            let recovery = self.recover_pending_input_overflow().map_err(|recovery| {
                CsharpProductRuntimeError::new(
                    "CSHARP_INPUT_RECOVERY",
                    format!("{}: {}", recovery.code(), recovery.diagnostic()),
                )
            })?;
            let (_, outputs) = recovery.into_parts();
            // `input` has no operation-output channel of its own. Keep the
            // latest complete baseline so the following scheduled operation
            // publishes the new binding instead of leaving the browser stale.
            self.pending_recovery_outputs = outputs;
            let _ = self.diagnostics.publish(
                ProductHostLogEvent::new(
                    ProductHostLogSeverity::Warning,
                    ProductHostLogDisposition::ResyncRequired,
                    "csharp-runtime",
                    error.code(),
                    error.detail(),
                )
                .expect("bounded input recovery diagnostic")
                .with_runtime(self.binding()),
            );
            return Ok(PendingInputAdmission::Resynchronized);
        }
        self.pending_inputs.extend(inputs);
        Ok(PendingInputAdmission::Appended)
    }

    fn action<F, T>(
        &mut self,
        action: NativeProductAction,
        operation: ProductHostOperationKind,
        transition: F,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError>
    where
        F: FnOnce(&mut RuntimeLifecycle) -> Result<T, runtime_lifecycle::RuntimeLifecycleError>,
    {
        let call_binding = ui_binding(&self.lifecycle);
        if matches!(
            operation,
            ProductHostOperationKind::Start
                | ProductHostOperationKind::Pause
                | ProductHostOperationKind::Resume
                | ProductHostOperationKind::Restart
        ) {
            self.services.begin_lifecycle_call(call_binding);
        } else {
            self.services.begin_call(call_binding);
        }
        let callback_result = call_action(&self.api, action, self.handle, operation);
        let finished = self.finish_product_call(callback_result.err());
        let mut call_outputs = finished.outputs;
        // The product's own lifecycle callback ran (or threw); the transition
        // still applies, so a failure lands in the lifecycle's new state.
        transition(&mut self.lifecycle).map_err(lifecycle_error)?;
        // After the transition, so that a Restart callback chooses the new
        // generation's gameplay time.
        self.settle_gameplay_time();
        self.follow_world_time();
        if let Some(failure) = finished.failure {
            self.fault_after_call(
                operation_name(operation),
                Some(&failure),
                finished.input_mapping_replacement,
                &mut call_outputs,
            )?;
            observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
            return Ok(call_outputs);
        }
        let binding = ui_binding(&self.lifecycle);
        let mut outputs = if matches!(operation, ProductHostOperationKind::Start) {
            self.initial_output.take().unwrap_or_default()
        } else {
            Vec::new()
        };
        outputs.extend(call_outputs);
        if binding != call_binding {
            // The browser clears UI when the binding changes. Each stream's
            // current projection follows the new binding, whether or not the
            // lifecycle callback published one, as in the other rebinds.
            outputs.retain(|output| !matches!(output, RuntimePublication::UiProjection(_)));
            outputs.extend(
                self.services
                    .snapshot_ui_projections(binding)
                    .into_iter()
                    .map(RuntimePublication::UiProjection),
            );
        }
        if let Some(replacement) = finished.input_mapping_replacement {
            self.input_lane
                .replace_physical_mappings(replacement)
                .map_err(input_error)?;
        }
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        Ok(outputs)
    }

    fn snapshot_outputs(&self) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        let mut outputs = service_outputs(
            self.services
                .snapshot_outputs(ui_binding(&self.lifecycle))?,
        )?;
        if self.audio_output.is_some() {
            audio_output::take_audio_ops(&mut outputs);
        }
        Ok(outputs)
    }

    fn receipt(
        &mut self,
        operation: ProductHostOperationKind,
        outputs: Vec<RuntimePublication>,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        let mut all_outputs = self.take_pending_recovery_outputs();
        all_outputs.extend(outputs);
        let readout = self.readout();
        let result = ProductHostOperationResult::accepted(
            operation,
            self.binding(),
            self.next_input_sequence(),
            readout,
        )
        .map_err(host_runtime_error)?;
        ProductHostRuntimeReceipt::new(result, all_outputs).map_err(host_runtime_error)
    }

    fn take_pending_recovery_outputs(&mut self) -> Vec<RuntimePublication> {
        std::mem::take(&mut self.pending_recovery_outputs)
    }

    /// A lifecycle admission advances Rust-owned counters before the product
    /// callback is entered. Once the following update path fails, neither the
    /// callback's product state nor the retained Engine service state can be
    /// assumed to be replay-safe. Return a current receipt instead of making
    /// the host retry an operation which may already have been consumed.
    fn resync_operation(
        &mut self,
        operation: ProductHostOperationKind,
        error: CsharpProductRuntimeError,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        let error = ProductHostRuntimeError::new(error.code(), error.detail().to_owned());
        self.resync_operation_runtime_error(operation, error)
    }

    fn resync_operation_runtime_error(
        &mut self,
        operation: ProductHostOperationKind,
        error: ProductHostRuntimeError,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        // The admitted update has crossed the callback boundary or consumed
        // its input snapshot. Do not leave those borrowed native events queued
        // for a later caller to replay after this resync receipt.
        if self.pending_recovery_outputs.is_empty() {
            self.pending_inputs.clear();
        }
        self.publish_diagnostic_as(&error, ProductHostFaultDisposition::ResyncRequired);
        let result = ProductHostOperationResult::resync_required(
            operation,
            self.binding(),
            self.next_input_sequence(),
            self.readout(),
            self.lifecycle
                .readout()
                .admitted_simulation_steps()
                .checked_sub(1)
                .map(CanonicalU64::new),
            error.code().to_owned(),
            error.diagnostic().to_owned(),
        )
        .map_err(host_runtime_error)?;
        ProductHostRuntimeReceipt::new(result, self.take_pending_recovery_outputs())
            .map_err(host_runtime_error)
    }

    fn readout(&self) -> ProductHostRuntimeReadout {
        dev_readout(self.lifecycle.readout())
    }

    fn runtime_error(&self, error: CsharpProductRuntimeError) -> ProductHostRuntimeError {
        let runtime_error = ProductHostRuntimeError::new(error.code(), error.detail().to_owned());
        self.publish_diagnostic(&runtime_error);
        runtime_error
    }

    fn lifecycle_runtime_error(
        &self,
        error: runtime_lifecycle::RuntimeLifecycleError,
    ) -> ProductHostRuntimeError {
        let runtime_error = lifecycle_runtime_error(error);
        self.publish_diagnostic(&runtime_error);
        runtime_error
    }

    fn publish_diagnostic(&self, error: &ProductHostRuntimeError) {
        self.publish_diagnostic_as(error, runtime_fault_disposition(error));
    }

    fn publish_diagnostic_as(
        &self,
        error: &ProductHostRuntimeError,
        disposition: ProductHostFaultDisposition,
    ) {
        let message = if error.diagnostic().is_empty() {
            "runtime operation failed"
        } else {
            error.diagnostic()
        };
        let _ = self.diagnostics.publish(
            ProductHostLogEvent::new(
                ProductHostLogSeverity::Error,
                match disposition {
                    ProductHostFaultDisposition::Accepted => ProductHostLogDisposition::Accepted,
                    ProductHostFaultDisposition::RejectedRecoverable => {
                        ProductHostLogDisposition::RejectedRecoverable
                    }
                    ProductHostFaultDisposition::Degraded => ProductHostLogDisposition::Degraded,
                    ProductHostFaultDisposition::ResyncRequired => {
                        ProductHostLogDisposition::ResyncRequired
                    }
                    ProductHostFaultDisposition::Terminal => ProductHostLogDisposition::Terminal,
                },
                "csharp-runtime",
                error.code(),
                message,
            )
            .expect("runtime diagnostics are bounded")
            .with_runtime(self.binding()),
        );
    }

    fn binding(&self) -> ProductHostRuntimeBinding {
        dev_binding(self.lifecycle.readout())
    }

    fn next_input_sequence(&self) -> CanonicalU64 {
        CanonicalU64::new(
            self.input_lane
                .last_sequence()
                .map_or(0, |sequence| sequence.saturating_add(1)),
        )
    }

    fn require_control_binding(
        &self,
        operation: ProductHostLifecycleOperation,
        binding: Option<ProductHostRuntimeBinding>,
    ) -> Result<(), ProductHostRuntimeError> {
        if operation == ProductHostLifecycleOperation::Start
            && self.lifecycle.state() == RuntimeState::Created
            && binding.is_none_or(|value| value == self.binding())
        {
            return Ok(());
        }
        self.require_current_control_binding(binding)
    }

    /// The binding publication, carrying any harness's input claim.
    fn binding_output(&self) -> RuntimePublication {
        RuntimePublication::claimed_binding(
            input_binding(&self.lifecycle),
            self.next_input_sequence().get(),
            self.input_claim.as_ref().map(|claim| claim.label.clone()),
        )
    }

    /// Ends a harness's claim whose lease passed without input: a fresh
    /// binding clears what it held and hands input back to the page.
    fn expire_input_claim(&mut self) -> Result<Vec<RuntimePublication>, ProductHostRuntimeError> {
        if self
            .input_claim
            .as_ref()
            .is_none_or(|claim| claim.renewed.elapsed() < claim.lease)
        {
            return Ok(Vec::new());
        }
        self.input_claim = None;
        self.lifecycle
            .change_control(RuntimeControlOperation::Release)
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
            .map_err(|error| self.runtime_error(error))?;
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        self.rebind_in_place(Vec::new())
    }

    fn require_current_control_binding(
        &self,
        binding: Option<ProductHostRuntimeBinding>,
    ) -> Result<(), ProductHostRuntimeError> {
        if binding == Some(self.binding()) {
            return Ok(());
        }
        Err(ProductHostRuntimeError::new_not_applied(
            "CSHARP_CONTROL_BINDING",
            "lifecycle control does not name the current runtime binding",
        ))
    }

    /// Checks restart's lifecycle state without advancing any Engine-owned
    /// counter. The product callback must not run for an impossible host
    /// transition, because the Rust lifecycle remains the authority that
    /// decides whether a new generation can be admitted.
    fn require_restart_state(&self) -> Result<(), ProductHostRuntimeError> {
        if matches!(
            self.lifecycle.state(),
            RuntimeState::Running | RuntimeState::Paused | RuntimeState::Faulted
        ) {
            return Ok(());
        }
        Err(ProductHostRuntimeError::new_not_applied(
            "CSHARP_LIFECYCLE_ADMISSION",
            format!(
                "restart is not admitted from lifecycle state {:?}",
                self.lifecycle.state()
            ),
        ))
    }

    fn tag_complete_baseline(
        &self,
        outputs: Vec<RuntimePublication>,
    ) -> Result<Vec<RuntimePublication>, ProductHostRuntimeError> {
        self.rebind_outputs(outputs)
            .map_err(|error| self.runtime_error(error))
    }

    /// Reconstructs a complete browser baseline after an input binding fence.
    /// Retained UI and presentation state are rebound to the current runtime,
    /// while callback-local transient presentation stays in its original
    /// causal position before the baseline marker.
    fn rebind_outputs(
        &self,
        outputs: Vec<RuntimePublication>,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        let binding = self.binding();
        // Every binding baseline reconstructs committed state. Callback deltas
        // already precede its frontier and cannot be replayed against it.
        let mut snapshot = self.snapshot_outputs()?;
        for output in outputs {
            if let Some(events) = output.transient_presentation() {
                snapshot.push(events);
            }
        }
        let mut tagged = Vec::with_capacity(snapshot.len() + 2);
        tagged.push(self.binding_output());
        tagged.append(&mut snapshot);
        tagged.push(self.complete_baseline_output(binding)?);
        Ok(tagged)
    }

    /// Publishes a same-incarnation control fence (pause, resume, control
    /// replace/release, mapping replacement, input-overflow recovery) without
    /// rebuilding the world. The browser keeps its renderer and rebinds input,
    /// UI and feedback owners from the binding. The call's deltas follow the
    /// binding in causal order, and each UI stream's latest projection is
    /// republished under the new binding because the browser clears UI on a
    /// binding change.
    fn rebind_in_place(
        &self,
        outputs: Vec<RuntimePublication>,
    ) -> Result<Vec<RuntimePublication>, ProductHostRuntimeError> {
        self.rebind_outputs_in_place(outputs)
            .map_err(|error| self.runtime_error(error))
    }

    fn rebind_outputs_in_place(
        &self,
        outputs: Vec<RuntimePublication>,
    ) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
        let binding = self.binding();
        let mut tagged = Vec::with_capacity(outputs.len() + 2);
        tagged.push(self.binding_output());
        tagged.extend(
            outputs
                .into_iter()
                .filter(|output| !matches!(output, RuntimePublication::UiProjection(_))),
        );
        tagged.extend(
            self.services
                .snapshot_ui_projections(ui_binding(&self.lifecycle))
                .into_iter()
                .map(RuntimePublication::UiProjection),
        );
        tagged.push(self.complete_baseline_output(binding)?);
        Ok(tagged)
    }

    fn complete_baseline_output(
        &self,
        _binding: ProductHostRuntimeBinding,
    ) -> Result<RuntimePublication, CsharpProductRuntimeError> {
        let frontiers = self
            .services
            .renderer_publication_frontiers()
            .into_iter()
            .map(|(stream, revision)| RuntimePublicationFrontier::new(stream, revision))
            .collect::<Result<Vec<_>, _>>()
            .map_err(publication_error)?;
        Ok(RuntimePublication::complete_baseline_with_frontiers(
            input_binding(&self.lifecycle),
            frontiers,
        ))
    }

    /// Rebind input for a transition that replaces the browser's world (a
    /// new generation or a complete baseline): the replacement renderer
    /// starts fresh realization owners, so Rust forgets their facts too.
    fn rebind_input(&mut self, reason: InputClearReason) -> Result<(), CsharpProductRuntimeError> {
        self.rebind_input_in_place(reason)?;
        self.services.reset_audio_realization_owner();
        if let Some(audio) = &mut self.audio_output {
            audio.rebaseline(&self.services)?;
        }
        self.rebaseline_frames();
        self.follow_world_time();
        self.services.reset_animation_realization_owner();
        self.services.reset_ghost_plate_realization_owner();
        Ok(())
    }

    /// Whether host time becomes world time now: the runtime runs, playtest
    /// inspection does not hold it, and the product's gameplay time is not
    /// held.
    fn world_time_moves(&self) -> bool {
        self.lifecycle.state() == RuntimeState::Running
            && self.playtest_time == playtest::TimeMode::Realtime
            && !self.lifecycle.gameplay_time().held()
    }

    /// Device audio plays only while world time moves, and at its rate:
    /// Engine cursors advance only with admitted steps, so the device holds
    /// and slows with them. Shutdown ends every voice.
    pub(crate) fn follow_world_time(&mut self) {
        self.follow_simulation_with_frames();
        let moves = self.world_time_moves();
        let rate =
            csharp_engine_services::gameplay_rate_value(self.lifecycle.gameplay_time().rate());
        let Some(audio) = &mut self.audio_output else {
            return;
        };
        match self.lifecycle.state() {
            RuntimeState::Shutdown => audio.silence(),
            _ => audio.follow_world(!moves, rate),
        }
    }

    /// Tells the renderer the step it shows and whether world time is held,
    /// which its frame and capture readouts report.
    pub(crate) fn follow_simulation_with_frames(&self) {
        if let Some(frames) = &self.frame_output {
            frames.follow_simulation(self.frame_simulation());
        }
    }

    /// Writes the committed scene, as a fresh renderer would be built from
    /// it, with every renderer resource the Engine holds
    /// (`engine.renderer.snapshot <path>`). A relative path resolves against
    /// the host's working directory.
    fn write_scene_snapshot(
        &self,
        path: &str,
    ) -> Result<scene_snapshot::SceneSnapshotReport, String> {
        if path.is_empty() {
            return Err(format!("usage: {SCENE_SNAPSHOT_COMMAND} <path>"));
        }
        let baseline = self
            .services
            .snapshot_outputs(ui_binding(&self.lifecycle))
            .map_err(CsharpProductRuntimeError::from)
            .and_then(service_outputs)
            .map_err(|error| format!("{}: {}", error.code(), error.detail()))?;
        let metadata = scene_snapshot::SceneSnapshotMetadata {
            product: self.product.clone(),
            host: scene_snapshot::SceneSnapshotHost {
                version: env!("CARGO_PKG_VERSION").to_owned(),
                abi_fingerprint: product_host_runtime_identity().fingerprint_hex(),
            },
            adapter: self.frame_output.as_ref().map(|frames| {
                let adapter = frames.driver().gpu().adapter_summary();
                format!("{} ({})", adapter.name, adapter.backend)
            }),
            written_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64),
            state: frame_output::scene_state(&self.services, self.frame_simulation()).into(),
            options: self.renderer_options.into(),
        };
        let resources = self
            .services
            .renderer_resources()
            .map(|resource| (resource.identity(), resource.bytes()));
        scene_snapshot::write_scene_snapshot(Path::new(path), &metadata, &baseline, resources)
    }

    fn frame_simulation(&self) -> frame_output::Simulation {
        frame_output::Simulation {
            held: !self.world_time_moves(),
            step: self.lifecycle.readout().admitted_simulation_steps(),
        }
    }

    /// Rebuilds the streamed renderer from the committed world. Renderer
    /// work a failed call did not publish would otherwise be missing from it.
    fn rebaseline_frames(&self) {
        let Some(frames) = &self.frame_output else {
            return;
        };
        match self
            .services
            .snapshot_outputs(ui_binding(&self.lifecycle))
            .map_err(CsharpProductRuntimeError::from)
            .and_then(service_outputs)
        {
            Ok(baseline) => frames.rebaseline(&self.services, &baseline, self.frame_simulation()),
            Err(error) => {
                let _ = self.diagnostics.publish(
                    ProductHostLogEvent::new(
                        ProductHostLogSeverity::Warning,
                        ProductHostLogDisposition::Degraded,
                        "csharp-runtime",
                        error.code(),
                        error.detail(),
                    )
                    .expect("bounded frame baseline diagnostic")
                    .with_runtime(self.binding()),
                );
            }
        }
    }

    /// The images and fonts the product granted its UI, which the host serves to the page.
    pub fn ui_files(&self) -> product_host::ProductHostUiFiles {
        let files = self.services.ui_files();
        Arc::new(move |id| files.get(id).map(|file| (file.content_type, file.bytes)))
    }

    /// The host serves these frames when this process streams the world.
    /// Where the page reports how it presents the product.
    pub fn presentation(&self) -> Arc<product_host::ProductHostPresentation> {
        Arc::clone(&self.presentation)
    }

    /// The mixed audio the watching pages play, when audio output is
    /// `stream`.
    pub fn audio_stream(&self) -> Option<Arc<product_host::ProductHostAudioStream>> {
        self.audio_output
            .as_ref()
            .and_then(audio_output::AudioOutput::stream)
    }

    pub fn frame_stream(&self) -> Option<Arc<product_host::ProductHostFrameStream>> {
        self.frame_output
            .as_ref()
            .and_then(frame_output::FrameOutput::frames)
    }

    /// Tool captures of the rendered world, in stream or window output.
    pub fn frame_capture(&self) -> Option<product_host::ProductHostFrameCapture> {
        self.frame_output
            .as_ref()
            .map(frame_output::FrameOutput::capture)
    }

    /// The renderer the desktop shell draws, in window output.
    pub fn scene_driver(&self) -> Option<Arc<SceneDriver>> {
        self.frame_output
            .as_ref()
            .map(frame_output::FrameOutput::driver)
    }

    /// Where the desktop shell reports its frames and input, in window
    /// output.
    pub fn window_timing(&self) -> Option<Arc<WindowTiming>> {
        self.frame_output
            .as_ref()
            .and_then(frame_output::FrameOutput::window_timing)
    }

    /// Rebind input for a same-incarnation control fence. The browser keeps
    /// its renderer, retained media and realization owners across it, so the
    /// realized facts stay valid here as well.
    fn rebind_input_in_place(
        &mut self,
        reason: InputClearReason,
    ) -> Result<(), CsharpProductRuntimeError> {
        let binding = input_binding(&self.lifecycle);
        self.input_lane
            .rebind(binding, standard_input_context(), reason)
            .map_err(input_error)?;
        self.pending_inputs.clear();
        self.pending_inputs.push(clear_input_owned(binding, reason));
        Ok(())
    }

    /// Settles a C#-staged mapping set only after the product callback and all
    /// other staged Engine output have committed. The existing control fence
    /// makes any browser batch carrying the old binding stale and gives the
    /// product one explicit clear before fresh physical edges are admitted.
    fn settle_input_mapping_replacement(
        &mut self,
        replacement: CompiledInputMappings,
    ) -> Result<(), CsharpProductRuntimeError> {
        self.input_lane
            .replace_physical_mappings(replacement)
            .map_err(input_error)?;
        self.lifecycle
            .change_control(RuntimeControlOperation::Replace)
            .map_err(lifecycle_error)?;
        self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
    }

    fn start_for_exercise(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let outputs = self.action(
            self.api.start,
            ProductHostOperationKind::Start,
            |lifecycle| lifecycle.start(),
        )?;
        assert_ui_projection_binding(&outputs, input_binding(&self.lifecycle))?;
        self.rebind_input(InputClearReason::Restart)
    }

    fn execute_renderer_debug(
        &mut self,
        action: RendererDebugCommand,
    ) -> Result<ProductHostDebugResult, ProductHostRuntimeError> {
        match action {
            RendererDebugCommand::Show => self.renderer_metrics_visible = true,
            RendererDebugCommand::Hide => self.renderer_metrics_visible = false,
            RendererDebugCommand::Toggle => {
                self.renderer_metrics_visible = !self.renderer_metrics_visible
            }
            RendererDebugCommand::Presentation
            | RendererDebugCommand::Read
            | RendererDebugCommand::Status => {}
        }
        if let (Some(frames), RendererDebugCommand::Presentation) = (&self.frame_output, action) {
            let presentation =
                frames.presentation(serde_json::to_value(self.binding()).unwrap_or_default());
            return Ok(ProductHostDebugResult::new(
                true,
                pretty_json(&presentation)?,
            ));
        }
        let status = ProductHostRendererStatus {
            available: self.frame_output.is_some(),
            widget: ProductHostRendererWidget {
                visible: self.renderer_metrics_visible,
            },
            diagnostic: self
                .frame_output
                .is_none()
                .then(|| "This runtime has no renderer.".to_owned()),
            renderer: self
                .frame_output
                .as_ref()
                .map(frame_output::FrameOutput::statistics),
        };
        let message = pretty_json(&status)?;
        // A read with no renderer is a failed observation for callers.
        // Visibility operations succeed either way, so a mounted widget can
        // show its unavailable state.
        let succeeded = match action {
            RendererDebugCommand::Read => status.available,
            RendererDebugCommand::Show
            | RendererDebugCommand::Hide
            | RendererDebugCommand::Toggle
            | RendererDebugCommand::Status
            | RendererDebugCommand::Presentation => true,
        };
        Ok(ProductHostDebugResult::new(succeeded, message))
    }
}

impl ProductHostRuntime for CsharpProductRuntime {
    /// Re-admits the staged bundle inventory so the next `OpenBundle` sees
    /// edited, added and deleted bundle files. Open bundles and content
    /// references keep the bytes they were opened with. The eager loose
    /// snapshot the product received at create is not reloaded; `rusty dev`
    /// replaces the runtime for loose content edits.
    fn reload_content(&mut self) -> Result<(), ProductHostRuntimeError> {
        let bundles = csharp_engine_services::ProductContentBundles::admit(
            &self.content_source,
            &self.content_root,
        )
        .map_err(|error| ProductHostRuntimeError::new("CSHARP_CONTENT_BUNDLES", error))?;
        self.services.bind_content_bundles(bundles);
        Ok(())
    }

    fn take_update_attribution(&mut self) -> Option<ProductHostUpdateAttribution> {
        self.pending_update_attribution.take()
    }

    fn realtime_schedule_state(&self) -> ProductHostRuntimeScheduleState {
        if !matches!(self.lifecycle.mode(), RuntimeMode::Realtime) {
            return ProductHostRuntimeScheduleState::Unsupported;
        }
        match self.lifecycle.state() {
            RuntimeState::Created => ProductHostRuntimeScheduleState::Created,
            RuntimeState::Running if self.playtest_time != playtest::TimeMode::Realtime => {
                ProductHostRuntimeScheduleState::Held
            }
            RuntimeState::Running => ProductHostRuntimeScheduleState::Running,
            RuntimeState::Paused => ProductHostRuntimeScheduleState::Paused,
            RuntimeState::Faulted => ProductHostRuntimeScheduleState::Faulted,
            RuntimeState::Shutdown => ProductHostRuntimeScheduleState::Shutdown,
        }
    }

    fn realtime_schedule_interval(&self) -> Option<std::time::Duration> {
        let RuntimeLifecycleConfig::Realtime(config) = self.lifecycle.configuration() else {
            return None;
        };
        // The host cadence is derived from the admitted lifecycle setting. The
        // standard 60 Hz value belongs only to standard_realtime_config(); it
        // is not a second scheduler policy here. Round up so an observation
        // deadline never precedes the exact fixed-step boundary.
        Some(std::time::Duration::from_nanos(
            1_000_000_000_u64.div_ceil(u64::from(config.fixed_step_hz())),
        ))
    }

    fn connect(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        if self.lifecycle.state() == RuntimeState::Created {
            return self.lifecycle_with_binding(ProductHostLifecycleOperation::Start, None);
        }
        if self.lifecycle.state() == RuntimeState::Shutdown {
            return Err(ProductHostRuntimeError::new_not_applied(
                "CSHARP_CONNECT_STATE",
                "a shutdown runtime cannot accept a browser connection",
            ));
        }
        self.receipt(
            ProductHostOperationKind::Connect,
            self.tag_complete_baseline(Vec::new())?,
        )
    }

    fn lifecycle(
        &mut self,
        operation: ProductHostLifecycleOperation,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.lifecycle_with_binding(operation, Some(self.binding()))
    }

    fn lifecycle_with_binding(
        &mut self,
        operation: ProductHostLifecycleOperation,
        binding: Option<ProductHostRuntimeBinding>,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.require_control_binding(operation, binding)?;
        match operation {
            ProductHostLifecycleOperation::Start => {
                let outputs = self
                    .action(
                        self.api.start,
                        ProductHostOperationKind::Start,
                        |lifecycle| lifecycle.start(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::Restart)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductHostOperationKind::Start,
                    self.tag_complete_baseline(outputs)?,
                )
            }
            ProductHostLifecycleOperation::Pause => {
                let outputs = self
                    .action(
                        self.api.pause,
                        ProductHostOperationKind::Pause,
                        |lifecycle| lifecycle.pause(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductHostOperationKind::Pause,
                    self.rebind_in_place(outputs)?,
                )
            }
            ProductHostLifecycleOperation::Resume => {
                let outputs = self
                    .action(
                        self.api.resume,
                        ProductHostOperationKind::Resume,
                        |lifecycle| lifecycle.resume(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductHostOperationKind::Resume,
                    self.rebind_in_place(outputs)?,
                )
            }
            ProductHostLifecycleOperation::Restart => {
                // Keep the callback-first ordering used by the other product
                // lifecycle actions, but validate the Rust-owned state before
                // entering C#. A callback failure therefore leaves the
                // authoritative lifecycle binding and generation untouched.
                self.require_restart_state()?;
                let outputs = self
                    .action(
                        self.api.restart,
                        ProductHostOperationKind::Restart,
                        |lifecycle| lifecycle.restart(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::Restart)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductHostOperationKind::Restart,
                    self.tag_complete_baseline(outputs)?,
                )
            }
            ProductHostLifecycleOperation::ReportFault => {
                // Fault reporting is a host control, not a reentrant product
                // callback. RuntimeLifecycle preserves its counters while
                // advancing the control revision and recording the typed
                // owner fault.
                self.lifecycle
                    .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
                    .map_err(|error| self.lifecycle_runtime_error(error))?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
                self.receipt(
                    ProductHostOperationKind::ReportFault,
                    self.tag_complete_baseline(Vec::new())?,
                )
            }
            ProductHostLifecycleOperation::Shutdown => {
                let outputs = self
                    .action(
                        self.api.shutdown,
                        ProductHostOperationKind::Shutdown,
                        |lifecycle| lifecycle.shutdown(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.input_lane.dispose();
                self.pending_inputs.clear();
                // Once the callback transaction and Rust lifecycle transition
                // have both committed, Drop must not replay Shutdown merely
                // because serializing the host receipt later fails.
                self.shutdown_called = true;
                self.receipt(
                    ProductHostOperationKind::Shutdown,
                    self.tag_complete_baseline(outputs)?,
                )
            }
        }
    }

    fn control(
        &mut self,
        operation: ProductHostControlOperation,
        binding: ProductHostRuntimeBinding,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.require_current_control_binding(Some(binding))?;
        let lifecycle_operation = match operation {
            ProductHostControlOperation::Replace => RuntimeControlOperation::Replace,
            ProductHostControlOperation::Release => {
                // Release ends a harness's claim; the page takes input back.
                self.input_claim = None;
                RuntimeControlOperation::Release
            }
        };
        self.lifecycle
            .change_control(lifecycle_operation)
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
            .map_err(|error| self.runtime_error(error))?;
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        self.receipt(
            operation.operation_kind(),
            self.rebind_in_place(Vec::new())?,
        )
    }

    fn claim_control(
        &mut self,
        binding: ProductHostRuntimeBinding,
        label: String,
        lease: std::time::Duration,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.require_current_control_binding(Some(binding))?;
        if label.trim().is_empty() || label.len() > 64 || label.chars().any(char::is_control) {
            return Err(ProductHostRuntimeError::new_not_applied(
                "CSHARP_CONTROL_CLAIM",
                "a claim label is 1..=64 bytes of visible text",
            ));
        }
        if lease.is_zero() || lease > MAX_INPUT_CLAIM_LEASE {
            return Err(ProductHostRuntimeError::new_not_applied(
                "CSHARP_CONTROL_CLAIM",
                "a claim lease is 1 ms to one hour",
            ));
        }
        self.lifecycle
            .change_control(RuntimeControlOperation::Replace)
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        self.input_claim = Some(InputClaim {
            label,
            lease,
            renewed: std::time::Instant::now(),
        });
        self.rebind_input_in_place(InputClearReason::ControlRevisionChange)
            .map_err(|error| self.runtime_error(error))?;
        observe_product_runtime(&self.api, self.handle, self.lifecycle.readout());
        self.receipt(
            ProductHostOperationKind::ClaimControl,
            self.rebind_in_place(Vec::new())?,
        )
    }

    fn input(
        &mut self,
        batch: ProductHostInputBatch,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostInputResult>, ProductHostRuntimeError> {
        // Input under a claim is the harness's: it keeps the lease alive.
        if let Some(claim) = &mut self.input_claim {
            claim.renewed = std::time::Instant::now();
        }
        // A page keeps sending input while the product is paused (a menu's
        // clears, say). It is admitted for the page's cursor; physical input
        // is dropped, and resume rebinds input, so Update never sees the
        // paused interval. Direct UI claims reach the product once, now.
        let paused = self.lifecycle.state() == RuntimeState::Paused;
        if !paused && self.lifecycle.state() != RuntimeState::Running {
            return Err(ProductHostRuntimeError::new_not_applied(
                "CSHARP_INPUT_STATE",
                "input is admitted only while the standard runtime is running or paused",
            ));
        }
        // RuntimeInputLane owns the checkpoint. This keeps a valid prefix from
        // reaching the product when a later event is malformed or from a
        // foreign binding, and gives the host a precise safe-drop cursor.
        let receipt = self
            .input_lane
            .ingest_batch(batch.events())
            .map_err(input_runtime_error)?;
        // A safe drop is a batch a fence made stale or a replay behind the
        // cursor: expected, and answered in the receipt, so it is recorded as
        // information. (The sink raises any rejected-recoverable event to a
        // warning.)
        if receipt.dropped_count() > 0 {
            let _ = self.diagnostics.publish(
                ProductHostLogEvent::new(
                    ProductHostLogSeverity::Info,
                    ProductHostLogDisposition::Accepted,
                    "csharp-runtime",
                    "CSHARP_INPUT_STALE_DROPPED",
                    format!(
                        "dropped {} stale or duplicate input event(s); the current input cursor remains authoritative",
                        receipt.dropped_count()
                    ),
                )
                .expect("stale input diagnostic is bounded")
                .with_runtime(self.binding()),
            );
        }
        // No mapped intent or held key from the paused interval is kept for
        // a later step.
        let paused_intents = if paused {
            self.input_lane.take_paused_direct_intents()
        } else {
            Vec::new()
        };
        let native = if paused {
            Vec::new()
        } else {
            receipt
                .accepted_indices()
                .iter()
                .filter(|index| matches!(batch.events()[**index], RuntimeInputEvent::Physical(_)))
                .map(|index| native_event(&batch.events()[*index]))
                .collect::<Vec<_>>()
        };
        if self
            .append_pending_inputs(native)
            .map_err(|error| self.runtime_error(error))?
            == PendingInputAdmission::Resynchronized
        {
            let result = ProductHostInputResult::pending_resynchronized(
                receipt.submitted_count(),
                self.next_input_sequence(),
                self.binding(),
                self.readout(),
            )
            .map_err(host_runtime_error)?;
            return ProductHostRuntimeReceipt::new(
                result,
                std::mem::take(&mut self.pending_recovery_outputs),
            )
            .map_err(host_runtime_error);
        }
        let (outputs, faulted) = self
            .deliver_paused_intents(paused_intents)
            .map_err(|error| self.runtime_error(error))?;
        // A product failure rebinds input, so the lane's cursor is current.
        let next_input_sequence = receipt
            .next_sequence()
            .filter(|_| !faulted)
            .map(CanonicalU64::new)
            .unwrap_or_else(|| self.next_input_sequence());
        let result = ProductHostInputResult::with_progress(
            receipt.submitted_count(),
            receipt.accepted_count(),
            receipt.dropped_count(),
            receipt.accepted_through().map(CanonicalU64::new),
            receipt.consumed_through().map(CanonicalU64::new),
            next_input_sequence,
            self.binding(),
            self.readout(),
        )
        .map_err(host_runtime_error)?;
        ProductHostRuntimeReceipt::new(result, outputs).map_err(host_runtime_error)
    }

    fn recover_input_overflow(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        self.recover_pending_input_overflow()
    }

    fn execute_debug(
        &mut self,
        command: &str,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
        if command.split_whitespace().next().is_some_and(|name| {
            matches!(
                name,
                "engine.time" | "engine.time.mode" | "engine.time.advance"
            )
        }) {
            return self.execute_time_debug(command);
        }
        if let Some(frames) = self
            .frame_output
            .as_ref()
            .filter(|_| frame_output::is_inspection_command(command))
        {
            let result = match frames.execute_inspection(command) {
                Ok(answer) => ProductHostDebugResult::new(true, pretty_json(&answer)?),
                Err(detail) => ProductHostDebugResult::new(false, detail),
            };
            return ProductHostRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error);
        }
        if let Some(path) = command
            .trim()
            .strip_prefix(SCENE_SNAPSHOT_COMMAND)
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
        {
            let result = match self.write_scene_snapshot(path.trim()) {
                Ok(report) => ProductHostDebugResult::new(true, pretty_json(&report)?),
                Err(detail) => ProductHostDebugResult::new(false, detail),
            };
            return ProductHostRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error);
        }
        if let Some(action) = renderer_debug_command(command) {
            let result = self.execute_renderer_debug(action)?;
            return ProductHostRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error);
        }
        let (execute, release) = (self.api.execute_debug, self.api.release_debug_result);

        // Debug commands may use ordinary generated Engine services, and their
        // changes stay like any other call's.
        self.services.begin_call(ui_binding(&self.lifecycle));
        let callback_result = call_debug(execute, release, self.handle, command);
        let finished = self.finish_product_call(callback_result.as_ref().err().cloned());
        self.settle_gameplay_time();
        match (callback_result, finished.failure) {
            (Ok(result), None) => {
                ProductHostRuntimeReceipt::new(result, finished.outputs).map_err(host_runtime_error)
            }
            (_, Some(failure)) => {
                // The next receipt carries what the command did publish.
                self.pending_recovery_outputs.extend(finished.outputs);
                Err(self.runtime_error(failure))
            }
            (Err(_), None) => unreachable!("a callback error is the call's failure"),
        }
    }

    fn describe_debug(
        &mut self,
    ) -> Result<
        ProductHostRuntimeReceipt<product_host::ProductHostDebugCatalog>,
        ProductHostRuntimeError,
    > {
        let result = call_describe_debug(
            self.api.describe_debug,
            self.api.release_debug_result,
            self.handle,
        )
        .map_err(|error| self.runtime_error(error))?;
        let catalog =
            product_host::ProductHostDebugCatalog::decode_json(result.message().as_bytes())
                .map_err(|error| {
                    self.runtime_error(CsharpProductRuntimeError::new(
                        error.code(),
                        error.detail().to_owned(),
                    ))
                })?;
        let catalog = catalog.with_renderer_diagnostics();
        let catalog = if self.frame_output.is_some() {
            catalog.with_runtime_renderer_inspection()
        } else {
            catalog
        };
        ProductHostRuntimeReceipt::new(catalog, Vec::new()).map_err(host_runtime_error)
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        // An expired claim's release is this observation's whole output; the
        // next observation simulates.
        let released = self.expire_input_claim()?;
        if !released.is_empty() || self.playtest_time != playtest::TimeMode::Realtime {
            return self.receipt(ProductHostOperationKind::AdvanceRealtime, released);
        }
        let admission = self
            .lifecycle
            .advance_realtime(HostMonotonicTime::from_nanoseconds(observed_time_ns.get()))
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        // A bounded advance counts down (and ends in a hold) here.
        self.settle_gameplay_time();
        self.host_elapsed_ns = match admission.elapsed_nanoseconds() {
            Some(elapsed) => self.host_elapsed_ns.saturating_add(elapsed),
            None => 0,
        };
        let outputs = match admission.simulation() {
            // The lifecycle owns admission and its readout counters. Runtime
            // Input snapshots once with the last admitted phase token; the
            // product receives one update per accepted host observation while
            // retaining the host observation as its realtime timing value.
            Some(simulation) => match self.update_admitted(
                REALTIME_UPDATE_MODE,
                Some(observed_time_ns.get()),
                simulation,
                admission.dropped_steps(),
            ) {
                Ok(outputs) => outputs,
                Err(error) => {
                    return self.resync_operation(ProductHostOperationKind::AdvanceRealtime, error);
                }
            },
            // A product that selected gameplay time updates at every
            // observation, so it can look, present and choose while held.
            None if self.lifecycle.gameplay_time().selected() => {
                match self.update_without_step(observed_time_ns.get()) {
                    Ok(outputs) => outputs,
                    Err(error) => {
                        return self
                            .resync_operation(ProductHostOperationKind::AdvanceRealtime, error);
                    }
                }
            }
            None => Vec::new(),
        };
        match self.receipt(ProductHostOperationKind::AdvanceRealtime, outputs) {
            Ok(receipt) => Ok(receipt),
            Err(error) => self
                .resync_operation_runtime_error(ProductHostOperationKind::AdvanceRealtime, error),
        }
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        let admission = self
            .lifecycle
            .admit_demand_step()
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        let outputs = match self.update_admitted(DEMAND_UPDATE_MODE, None, admission, 0) {
            Ok(outputs) => outputs,
            Err(error) => {
                return self.resync_operation(ProductHostOperationKind::AdmitDemandStep, error);
            }
        };
        match self.receipt(ProductHostOperationKind::AdmitDemandStep, outputs) {
            Ok(receipt) => Ok(receipt),
            Err(error) => self
                .resync_operation_runtime_error(ProductHostOperationKind::AdmitDemandStep, error),
        }
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostOperationResult>, ProductHostRuntimeError>
    {
        let admission = self
            .lifecycle
            .admit_external_step(ExternalStep::new(step.get()))
            .map_err(|error| self.lifecycle_runtime_error(error))?;
        let outputs = match self.update_admitted(EXTERNAL_UPDATE_MODE, None, admission, 0) {
            Ok(outputs) => outputs,
            Err(error) => {
                return self.resync_operation(ProductHostOperationKind::AdmitExternalStep, error);
            }
        };
        match self.receipt(ProductHostOperationKind::AdmitExternalStep, outputs) {
            Ok(receipt) => Ok(receipt),
            Err(error) => self
                .resync_operation_runtime_error(ProductHostOperationKind::AdmitExternalStep, error),
        }
    }

    fn complete_timeline(
        &mut self,
        completion: ProductHostTimelineCompletion,
    ) -> Result<
        ProductHostRuntimeReceipt<ProductHostTimelineCompletionResult>,
        ProductHostRuntimeError,
    > {
        let envelope = completion.envelope();
        let ticket = CanonicalU64::new(envelope.ticket().value());
        let binding = envelope.binding();
        let current = self.binding();
        if self.lifecycle.state() != RuntimeState::Running
            || binding.instance_id().value() != current.instance_id.get()
            || binding.generation().value() != current.generation.get()
            || binding.control_revision().value() != current.control_revision.get()
        {
            let result = ProductHostTimelineCompletionResult::rejected_with_current(
                ticket,
                current,
                self.readout(),
                "CSHARP_TIMELINE_BINDING",
                "timeline completion does not name the current running product binding",
            )
            .map_err(host_runtime_error)?;
            return ProductHostRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error);
        }

        let outcome_data = match envelope.outcome() {
            product_host::TimelineCompletionOutcome::Success(data)
            | product_host::TimelineCompletionOutcome::Failure(data) => data
                .as_ref()
                .map(|value| serde_json::to_vec(value.value()))
                .transpose()
                .map_err(|error| {
                    ProductHostRuntimeError::new(
                        "CSHARP_TIMELINE_DATA",
                        format!("timeline outcome data could not be copied: {error}"),
                    )
                })?,
        };
        let provenance_detail = envelope
            .provenance()
            .detail()
            .map(|value| serde_json::to_vec(value.value()))
            .transpose()
            .map_err(|error| {
                ProductHostRuntimeError::new(
                    "CSHARP_TIMELINE_DATA",
                    format!("timeline provenance data could not be copied: {error}"),
                )
            })?;
        let native = NativeProductTimelineCompletion {
            ticket: ticket.get(),
            instance_id: current.instance_id.get(),
            generation: current.generation.get(),
            control_revision: current.control_revision.get(),
            correlation: native_utf8(envelope.correlation()),
            outcome: match envelope.outcome() {
                product_host::TimelineCompletionOutcome::Success(_) => {
                    NativeProductTimelineOutcome::Success
                }
                product_host::TimelineCompletionOutcome::Failure(_) => {
                    NativeProductTimelineOutcome::Failure
                }
            },
            outcome_data: native_optional_bytes(outcome_data.as_deref()),
            provenance_correlation: native_utf8(envelope.provenance().correlation()),
            provenance_detail: native_optional_bytes(provenance_detail.as_deref()),
        };

        self.services.begin_call(ui_binding(&self.lifecycle));
        let callback_result = call_complete_timeline(&self.api, self.handle, &native);
        let finished = self.finish_product_call(callback_result.as_ref().err().cloned());
        let mut outputs = finished.outputs;
        let result = if let Some(failure) = finished.failure {
            self.fault_after_call(
                "complete_timeline",
                Some(&failure),
                finished.input_mapping_replacement,
                &mut outputs,
            )
            .map_err(|error| self.runtime_error(error))?;
            ProductHostTimelineCompletionResult::rejected_with_current(
                ticket,
                self.binding(),
                self.readout(),
                failure.code(),
                failure.detail(),
            )
        } else if matches!(callback_result, Ok(true)) {
            ProductHostTimelineCompletionResult::accepted(ticket, self.binding(), self.readout())
        } else {
            ProductHostTimelineCompletionResult::rejected_with_current(
                ticket,
                self.binding(),
                self.readout(),
                "CSHARP_TIMELINE_PRODUCT_REJECTED",
                "the C# product did not accept the timeline completion",
            )
        }
        .map_err(host_runtime_error)?;
        ProductHostRuntimeReceipt::new(result, outputs).map_err(host_runtime_error)
    }
}

fn native_audio_diagnostic_code(
    code: render_presentation::AudioProjectionDiagnosticCode,
) -> NativeAudioDiagnosticCode {
    match code {
        render_presentation::AudioProjectionDiagnosticCode::InvalidDescriptor => {
            NativeAudioDiagnosticCode::InvalidDescriptor
        }
        render_presentation::AudioProjectionDiagnosticCode::AssetMissing => {
            NativeAudioDiagnosticCode::AssetMissing
        }
        render_presentation::AudioProjectionDiagnosticCode::AssetKindMismatch => {
            NativeAudioDiagnosticCode::AssetKindMismatch
        }
        render_presentation::AudioProjectionDiagnosticCode::ContentHashMismatch => {
            NativeAudioDiagnosticCode::ContentHashMismatch
        }
        render_presentation::AudioProjectionDiagnosticCode::DuplicateSignal => {
            NativeAudioDiagnosticCode::DuplicateSignal
        }
        render_presentation::AudioProjectionDiagnosticCode::DuplicateHandle => {
            NativeAudioDiagnosticCode::DuplicateHandle
        }
        render_presentation::AudioProjectionDiagnosticCode::UnknownHandle => {
            NativeAudioDiagnosticCode::UnknownHandle
        }
        render_presentation::AudioProjectionDiagnosticCode::UnavailableHost => {
            NativeAudioDiagnosticCode::UnavailableHost
        }
        render_presentation::AudioProjectionDiagnosticCode::AudioContextBlocked => {
            NativeAudioDiagnosticCode::AudioContextBlocked
        }
        render_presentation::AudioProjectionDiagnosticCode::DecodeFailed => {
            NativeAudioDiagnosticCode::DecodeFailed
        }
        render_presentation::AudioProjectionDiagnosticCode::HostFailure => {
            NativeAudioDiagnosticCode::HostFailure
        }
        render_presentation::AudioProjectionDiagnosticCode::InvalidControl => {
            NativeAudioDiagnosticCode::InvalidControl
        }
    }
}

impl Drop for CsharpProductRuntime {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        if !self.shutdown_called {
            // Implicit shutdown is an ordinary lifecycle action. Drop cannot
            // return a rejection, so report it on the process stream.
            if let Err(error) = self.action(
                self.api.shutdown,
                ProductHostOperationKind::Shutdown,
                |lifecycle| lifecycle.shutdown(),
            ) {
                eprintln!("CsharpProductRuntime implicit shutdown rejected: {error}");
            } else {
                self.input_lane.dispose();
                self.pending_inputs.clear();
                self.shutdown_called = true;
            }
        }
        // Shutdown silences the host's output, but a one-shot has no terminal
        // feedback after that silence. Clear the Engine-side realization
        // owner before Product Dispose gets its ordinary call so a product
        // can release its final clip references during disposal.
        self.services.reset_audio_realization_owner();
        // Product Dispose may release Engine resources, which it does inside
        // an ordinary call like any other.
        self.services.begin_call(ui_binding(&self.lifecycle));
        // SAFETY: destroy runs exactly once before the `Library` field drops.
        unsafe { (self.api.destroy)(self.handle) };
        let _ = self.services.finish_call();
        self.handle = ptr::null_mut();
        // A NativeAOT shared library may retain runtime worker infrastructure
        // beyond its exported destroy function. Process-lifetime mapping keeps
        // Drop safe while preserving the required product destroy ordering.
        if let LoadedProductHost::NativeAot(library) = &mut self.api.host {
            if let Some(library) = library.take() {
                std::mem::forget(library);
            }
        }
    }
}

fn dev_binding(readout: RuntimeLifecycleReadout) -> ProductHostRuntimeBinding {
    ProductHostRuntimeBinding {
        instance_id: CanonicalU64::new(readout.instance_id().value()),
        generation: CanonicalU64::new(readout.generation().value()),
        control_revision: CanonicalU64::new(readout.control_revision().value()),
    }
}

fn input_binding(lifecycle: &RuntimeLifecycle) -> RuntimeInputBinding {
    let readout = lifecycle.readout();
    RuntimeInputBinding::new(
        readout.instance_id(),
        readout.generation(),
        readout.control_revision(),
    )
}

fn ui_binding(lifecycle: &RuntimeLifecycle) -> RuntimeUiRuntimeBinding {
    RuntimeUiRuntimeBinding::from(lifecycle)
}

fn dev_binding_from_input(binding: RuntimeInputBinding) -> ProductHostRuntimeBinding {
    ProductHostRuntimeBinding {
        instance_id: CanonicalU64::new(binding.instance_id().value()),
        generation: CanonicalU64::new(binding.generation().value()),
        control_revision: CanonicalU64::new(binding.control_revision().value()),
    }
}

fn input_clear(binding: RuntimeInputBinding, sequence: u64) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        sequence,
        standard_input_context(),
        RuntimeInputFact::Clear {
            reason: InputClearReason::FocusLoss,
        },
    ))
}

fn key_press(binding: RuntimeInputBinding, sequence: u64) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        sequence,
        standard_input_context(),
        RuntimeInputFact::Key {
            code: runtime_input_model::KeyboardControl::KeyW,
            edge: runtime_input::PhysicalEdge::Pressed,
        },
    ))
}

fn fault_key_press(binding: RuntimeInputBinding, sequence: u64) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        sequence,
        standard_input_context(),
        RuntimeInputFact::Key {
            code: runtime_input_model::KeyboardControl::KeyF,
            edge: runtime_input::PhysicalEdge::Pressed,
        },
    ))
}

fn key_release(binding: RuntimeInputBinding, sequence: u64) -> RuntimeInputEvent {
    RuntimeInputEvent::Physical(RuntimeInputIngress::new(
        binding,
        sequence,
        standard_input_context(),
        RuntimeInputFact::Key {
            code: runtime_input_model::KeyboardControl::KeyW,
            edge: runtime_input::PhysicalEdge::Released,
        },
    ))
}

fn direct_intent(
    binding: RuntimeInputBinding,
    sequence: u64,
    descriptor: &DirectInputIntentDescriptor,
) -> Result<RuntimeInputEvent, CsharpProductRuntimeError> {
    let value = match descriptor.value_kind() {
        IntentValueKind::Digital => RuntimeIntentValue::Digital { active: true },
        IntentValueKind::Axis => RuntimeIntentValue::Axis {
            value: AxisValue::new(0.5).expect("fixed direct-intent exercise axis"),
        },
        IntentValueKind::ProductPayload => RuntimeIntentValue::ProductPayload {
            payload: runtime_input::RuntimeProductPayload::new(
                descriptor.payload_contract().ok_or_else(|| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_EXERCISE_DIRECT_INTENT",
                        "configured payload direct intent has no payload contract",
                    )
                })?,
                serde_json::json!({ "exercise": true }),
            )
            .map_err(input_error)?,
        },
    };
    RuntimeDirectIntentClaim::new(
        binding,
        sequence,
        standard_input_context(),
        descriptor.id(),
        value,
    )
    .map(RuntimeInputEvent::DirectIntent)
    .map_err(input_error)
}

fn payload_intent(
    binding: RuntimeInputBinding,
    sequence: u64,
    intent: &str,
    contract: &str,
) -> Result<RuntimeInputEvent, CsharpProductRuntimeError> {
    RuntimeDirectIntentClaim::new(
        binding,
        sequence,
        standard_input_context(),
        intent,
        RuntimeIntentValue::ProductPayload {
            payload: runtime_input::RuntimeProductPayload::new(
                contract,
                serde_json::json!({ "exercise": true }),
            )
            .map_err(input_error)?,
        },
    )
    .map(RuntimeInputEvent::DirectIntent)
    .map_err(input_error)
}

fn native_input_value_kind(value_kind: IntentValueKind) -> NativeInputValueKind {
    match value_kind {
        IntentValueKind::Digital => NativeInputValueKind::Digital,
        IntentValueKind::Axis => NativeInputValueKind::Axis,
        IntentValueKind::ProductPayload => NativeInputValueKind::ProductPayload,
    }
}

fn standard_input_context() -> InputContext {
    InputContext::new(STANDARD_INPUT_CONTEXT).expect("fixed standard input context")
}

fn dev_readout(readout: RuntimeLifecycleReadout) -> ProductHostRuntimeReadout {
    let mode = match readout.mode() {
        RuntimeMode::Realtime => product_host::ProductHostRuntimeMode::Realtime,
        RuntimeMode::Demand => product_host::ProductHostRuntimeMode::Demand,
        RuntimeMode::External => product_host::ProductHostRuntimeMode::External,
    };
    let state = match readout.state() {
        RuntimeState::Created => ProductHostRuntimeState::Created,
        RuntimeState::Running => ProductHostRuntimeState::Running,
        RuntimeState::Paused => ProductHostRuntimeState::Paused,
        RuntimeState::Faulted => ProductHostRuntimeState::Faulted,
        RuntimeState::Shutdown => ProductHostRuntimeState::Shutdown,
    };
    let mut projected = ProductHostRuntimeReadout::new(dev_binding(readout), mode, state)
        .with_counters(
            readout.admitted_simulation_steps(),
            readout.admitted_presentations(),
            readout.dropped_realtime_steps().min(u128::from(u64::MAX)) as u64,
            readout.clock_regressions(),
        )
        .with_clock(
            readout.scaled_remainder(),
            readout
                .last_observed_time()
                .map(|value| value.nanoseconds()),
        );
    if let Some(fault) = readout.fault() {
        projected = projected.with_fault(match fault {
            runtime_lifecycle::RuntimeFault::OwnerReported => {
                ProductHostRuntimeFault::OwnerReported
            }
            runtime_lifecycle::RuntimeFault::CounterExhausted => {
                ProductHostRuntimeFault::CounterExhausted
            }
        });
    }
    projected
}

fn update_facts(
    lifecycle: &RuntimeLifecycle,
    mode: NativeProductUpdateMode,
    observed_host_time_nanoseconds: Option<u64>,
    (simulation_step, admitted_step_count): (u64, u32),
    dropped_step_count: u128,
    host_elapsed_seconds: f64,
) -> Result<NativeProductUpdateFacts, CsharpProductRuntimeError> {
    let readout = lifecycle.readout();
    let gameplay = lifecycle.gameplay_time();
    let (observed_host_time_nanoseconds, fixed_step_hz, fixed_delta_seconds) =
        match lifecycle.configuration() {
            RuntimeLifecycleConfig::Realtime(config) => (
                observed_host_time_nanoseconds.unwrap_or_default(),
                config.fixed_step_hz(),
                1.0 / f64::from(config.fixed_step_hz()),
            ),
            RuntimeLifecycleConfig::Demand | RuntimeLifecycleConfig::External => (0, 0, 0.0),
        };
    let dropped_step_count = u64::try_from(dropped_step_count).map_err(|_| {
        CsharpProductRuntimeError::new(
            "CSHARP_LIFECYCLE_FACTS",
            "realtime dropped-step facts exceed the NativeAOT wire range",
        )
    })?;
    Ok(NativeProductUpdateFacts {
        mode,
        lifecycle_state: native_lifecycle_state(readout.state()),
        generation: readout.generation().value(),
        control_revision: readout.control_revision().value(),
        observed_host_time_nanoseconds,
        simulation_step,
        fixed_step_hz,
        admitted_step_count,
        dropped_step_count,
        fixed_delta_seconds,
        gameplay_time_selected: gameplay.selected(),
        gameplay_rate: csharp_engine_services::gameplay_rate_value(gameplay.rate()),
        gameplay_advance_remaining_steps: gameplay.advance_remaining_steps(),
        host_elapsed_seconds,
    })
}

fn native_lifecycle_state(state: RuntimeState) -> NativeProductLifecycleState {
    match state {
        RuntimeState::Created => NativeProductLifecycleState::Created,
        RuntimeState::Running => NativeProductLifecycleState::Running,
        RuntimeState::Paused => NativeProductLifecycleState::Paused,
        RuntimeState::Faulted => NativeProductLifecycleState::Faulted,
        RuntimeState::Shutdown => NativeProductLifecycleState::Shutdown,
    }
}

fn clear_input_owned(binding: RuntimeInputBinding, reason: InputClearReason) -> NativeInputOwned {
    let mut native = input_owned(
        0,
        binding,
        standard_input_context().as_str().as_bytes().to_vec(),
    );
    native.kind = NativeInputEventKind::Clear;
    native.device = NativeInputDevice::Runtime;
    native.channel = NativeInputChannel::Clear;
    native.clear_reason = clear_reason(reason);
    native.label = format!("{reason:?}").into_bytes();
    native
}

fn input_error(error: runtime_input::RuntimeInputError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new("CSHARP_INPUT_ADMISSION", error.to_string())
}

fn prepare_persistence_root(
    root: Option<&Path>,
) -> Result<Option<PathBuf>, CsharpProductRuntimeError> {
    let Some(root) = root else {
        return Ok(None);
    };
    if root.as_os_str().is_empty() || !root.is_absolute() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PERSISTENCE_ROOT",
            "persistence root must be an explicit absolute host path",
        ));
    }
    fs::create_dir_all(root).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_PERSISTENCE_ROOT",
            format!(
                "could not create persistence root {}: {error}",
                root.display()
            ),
        )
    })?;
    if !root.is_dir() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PERSISTENCE_ROOT",
            format!("persistence root {} is not a directory", root.display()),
        ));
    }
    Ok(Some(root.to_path_buf()))
}

fn input_runtime_error(error: runtime_input::RuntimeInputError) -> ProductHostRuntimeError {
    ProductHostRuntimeError::new_not_applied(input_error_code(&error), error.to_string())
}

fn lifecycle_error(error: runtime_lifecycle::RuntimeLifecycleError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(lifecycle_error_code(&error), error.to_string())
}

fn lifecycle_runtime_error(
    error: runtime_lifecycle::RuntimeLifecycleError,
) -> ProductHostRuntimeError {
    ProductHostRuntimeError::new_not_applied(lifecycle_error_code(&error), error.to_string())
}

fn lifecycle_error_code(error: &runtime_lifecycle::RuntimeLifecycleError) -> &'static str {
    use runtime_lifecycle::RuntimeLifecycleError;

    match error {
        RuntimeLifecycleError::WrongMode { .. } => "CSHARP_LIFECYCLE_WRONG_MODE",
        RuntimeLifecycleError::WrongState { .. } => "CSHARP_LIFECYCLE_WRONG_STATE",
        RuntimeLifecycleError::ClockRegression { .. } => "CSHARP_LIFECYCLE_CLOCK_REGRESSION",
        RuntimeLifecycleError::ExternalStepOutOfOrder { .. } => {
            "CSHARP_LIFECYCLE_EXTERNAL_STEP_OUT_OF_ORDER"
        }
        RuntimeLifecycleError::StaleToken { .. } => "CSHARP_LIFECYCLE_STALE_TOKEN",
        RuntimeLifecycleError::ForeignInstance { .. } => "CSHARP_LIFECYCLE_FOREIGN_INSTANCE",
        RuntimeLifecycleError::WrongPhaseToken { .. } => "CSHARP_LIFECYCLE_WRONG_PHASE_TOKEN",
        RuntimeLifecycleError::UnknownSimulationStep { .. } => {
            "CSHARP_LIFECYCLE_UNKNOWN_SIMULATION_STEP"
        }
        RuntimeLifecycleError::UnknownPresentation { .. } => {
            "CSHARP_LIFECYCLE_UNKNOWN_PRESENTATION"
        }
        RuntimeLifecycleError::GameplayAdvanceWithoutRate => {
            "CSHARP_LIFECYCLE_GAMEPLAY_ADVANCE_WITHOUT_RATE"
        }
        RuntimeLifecycleError::CounterExhausted => "CSHARP_LIFECYCLE_COUNTER_EXHAUSTED",
    }
}

fn input_error_code(error: &runtime_input::RuntimeInputError) -> &'static str {
    use runtime_input::RuntimeInputError;

    match error {
        RuntimeInputError::InvalidContext => "CSHARP_INPUT_INVALID_CONTEXT",
        RuntimeInputError::InvalidIntent => "CSHARP_INPUT_INVALID_INTENT",
        RuntimeInputError::InvalidAxisValue => "CSHARP_INPUT_INVALID_AXIS_VALUE",
        RuntimeInputError::InvalidDirectIntentAxisValue => {
            "CSHARP_INPUT_INVALID_DIRECT_INTENT_AXIS_VALUE"
        }
        RuntimeInputError::InvalidProductPayloadContract => {
            "CSHARP_INPUT_INVALID_PRODUCT_PAYLOAD_CONTRACT"
        }
        RuntimeInputError::ProductPayloadUnsafeInteger => {
            "CSHARP_INPUT_PRODUCT_PAYLOAD_UNSAFE_INTEGER"
        }
        RuntimeInputError::ProductPayloadContractMismatch => {
            "CSHARP_INPUT_PRODUCT_PAYLOAD_CONTRACT_MISMATCH"
        }
        RuntimeInputError::InvalidControllerAxisValue => {
            "CSHARP_INPUT_INVALID_CONTROLLER_AXIS_VALUE"
        }
        RuntimeInputError::InvalidControllerButtonValue => {
            "CSHARP_INPUT_INVALID_CONTROLLER_BUTTON_VALUE"
        }
        RuntimeInputError::NonCanonicalWireInteger => "CSHARP_INPUT_NON_CANONICAL_WIRE_INTEGER",
        RuntimeInputError::WireMalformed => "CSHARP_INPUT_WIRE_MALFORMED",
        RuntimeInputError::WireEventLimit => "CSHARP_INPUT_WIRE_EVENT_LIMIT",
        RuntimeInputError::SequenceOutOfOrder { .. } => "CSHARP_INPUT_SEQUENCE_OUT_OF_ORDER",
        RuntimeInputError::SequenceExhausted => "CSHARP_INPUT_SEQUENCE_EXHAUSTED",
        RuntimeInputError::BindingMismatch => "CSHARP_INPUT_BINDING_MISMATCH",
        RuntimeInputError::InvalidRebindClear => "CSHARP_INPUT_INVALID_REBIND_CLEAR",
        RuntimeInputError::PendingIngressOverflow => "CSHARP_INPUT_PENDING_INGRESS_OVERFLOW",
        RuntimeInputError::DuplicateIntent => "CSHARP_INPUT_DUPLICATE_INTENT",
        RuntimeInputError::InvalidMapping => "CSHARP_INPUT_INVALID_MAPPING",
        RuntimeInputError::DuplicateMapping => "CSHARP_INPUT_DUPLICATE_MAPPING",
        RuntimeInputError::MappingReplacementIntentMismatch => {
            "CSHARP_INPUT_MAPPING_REPLACEMENT_INTENT_MISMATCH"
        }
        RuntimeInputError::DirectIntentPayloadUnsupported => {
            "CSHARP_INPUT_DIRECT_INTENT_PAYLOAD_UNSUPPORTED"
        }
        RuntimeInputError::UnknownIntent => "CSHARP_INPUT_UNKNOWN_INTENT",
        RuntimeInputError::IntentValueKindMismatch => "CSHARP_INPUT_INTENT_VALUE_KIND_MISMATCH",
        RuntimeInputError::Disposed => "CSHARP_INPUT_DISPOSED",
        RuntimeInputError::WrongSnapshotPhase => "CSHARP_INPUT_WRONG_SNAPSHOT_PHASE",
        RuntimeInputError::LifecycleValidation => "CSHARP_INPUT_LIFECYCLE_VALIDATION",
        RuntimeInputError::SnapshotOutOfOrder => "CSHARP_INPUT_SNAPSHOT_OUT_OF_ORDER",
    }
}

fn exercise_runtime_error(error: ProductHostRuntimeError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(
        "CSHARP_EXERCISE",
        format!("{}: {}", error.code(), error.diagnostic()),
    )
}

fn call_create(
    api: &LoadedProductApi,
    args: &NativeProductCreateArgs,
    handle: &mut *mut c_void,
) -> Result<(), CsharpProductRuntimeError> {
    let mut native_error = NativeProductCallError::default();
    // SAFETY: fixed ABI pointers are valid for the duration of this call.
    let status = unsafe { (api.create)(args, handle, &mut native_error) };
    let diagnostic_result = if status == ABI_OK {
        if !native_product_call_error_is_empty(native_error) {
            Ok(Some(CsharpProductRuntimeError::new(
                "CSHARP_PRODUCT_CALL",
                "C# product create succeeded with a non-empty callback diagnostic",
            )))
        } else {
            Ok(None)
        }
    } else {
        copy_product_call_error(native_error)
    };
    // SAFETY: the generated product owns every buffer in the result and the
    // matching release is required for every invocation, including a failed
    // create with no product handle.
    unsafe { (api.release_call_error)(ptr::null_mut(), native_error) };
    if let Some(diagnostic) = diagnostic_result? {
        return Err(diagnostic);
    }
    checked_status(status, "create")
}

/// A generated C# callback can return before Rust's staged Engine services are
/// taken. Prefer the concrete service failure captured by those bridges, then
/// fall back to the product callback diagnostic when the callback failed for a
/// reason unrelated to an Engine service call. Taking the staged value here
/// is only an inspection/rollback step; callers still discard the transaction.
fn call_action(
    api: &LoadedProductApi,
    action: NativeProductAction,
    handle: *mut c_void,
    operation: ProductHostOperationKind,
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: `handle` is retained by the runtime.
    let status = unsafe { action(handle) };
    if status == ABI_OK {
        return Ok(());
    }
    let fallback = checked_status(status, operation_name(operation))
        .expect_err("non-success action status has a fallback error");
    Err(read_product_call_error(api, handle).unwrap_or(fallback))
}

fn call_update(
    api: &LoadedProductApi,
    handle: *mut c_void,
    args: NativeProductUpdateArgs,
) -> Result<NativeProductUpdateResult, CsharpProductRuntimeError> {
    let mut result = NativeProductUpdateResult::None;
    // SAFETY: event label pointers borrow local strings that remain alive for
    // the call; the C# product is required to copy anything it retains.
    let status = unsafe { (api.update)(handle, &args, &mut result) };
    if status != ABI_OK {
        let fallback = checked_status(status, "update")
            .expect_err("non-success update status has a fallback error");
        return Err(read_product_call_error(api, handle).unwrap_or(fallback));
    }
    Ok(result)
}

fn read_product_call_error(
    api: &LoadedProductApi,
    handle: *mut c_void,
) -> Option<CsharpProductRuntimeError> {
    let (read, release) = (api.read_call_error, api.release_call_error);
    let mut native_error = NativeProductCallError::default();
    // SAFETY: the product handle remains live for this immediate read and the
    // generated callback writes only the supplied result storage.
    let status = unsafe { read(handle, &mut native_error) };
    let copied = if status == ABI_OK {
        copy_product_call_error(native_error)
    } else {
        Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_CALL",
            format!("product callback error readout returned status {status}"),
        ))
    };
    // SAFETY: the product owns each result buffer and its matching release is
    // required even when the readout or its UTF-8 conversion failed.
    unsafe { release(handle, native_error) };
    copied.ok().flatten()
}

fn native_product_call_error_is_empty(error: NativeProductCallError) -> bool {
    error.status == 0
        && error.service.len == 0
        && error.operation.len == 0
        && error.message.len == 0
}

fn copy_product_call_error(
    native: NativeProductCallError,
) -> Result<Option<CsharpProductRuntimeError>, CsharpProductRuntimeError> {
    if native_product_call_error_is_empty(native) {
        return Ok(None);
    }
    let service = copy_product_error_text(native.service, "service")?;
    let operation = copy_product_error_text(native.operation, "operation")?;
    let message = copy_product_error_text(native.message, "message")?;
    let context = match (service.is_empty(), operation.is_empty()) {
        (false, false) => format!("{service}.{operation} returned status {}", native.status),
        (false, true) => format!("{service} returned status {}", native.status),
        (true, false) => format!("{operation} returned status {}", native.status),
        (true, true) => format!("product callback returned status {}", native.status),
    };
    let detail = if message.is_empty() {
        context
    } else {
        format!("{context}: {message}")
    };
    Ok(Some(CsharpProductRuntimeError::new(
        "CSHARP_PRODUCT_CALL",
        detail,
    )))
}

fn copy_product_error_text(
    value: NativeUtf8Slice,
    field: &str,
) -> Result<String, CsharpProductRuntimeError> {
    if value.len != 0 && value.bytes.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_DIAGNOSTIC_POINTER",
            format!("product callback {field} has length without bytes"),
        ));
    }
    // SAFETY: the product release callback keeps the returned bytes alive until
    // after this copy. A damaged encoding must not discard the product's error.
    let bytes = if value.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(value.bytes, value.len) }
    };
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

fn call_debug(
    execute: NativeProductExecuteDebug,
    release: NativeProductReleaseDebugResult,
    handle: *mut c_void,
    command: &str,
) -> Result<ProductHostDebugResult, CsharpProductRuntimeError> {
    let input = native_utf8(command);
    let mut native = NativeProductDebugResult::default();
    // SAFETY: `input` borrows `command` for this immediate call; `native` is
    // writable for the exact callback. The generated product must not retain
    // either borrowed input pointer.
    let status = unsafe { execute(handle, &input, &mut native) };
    // The callback owns any initialized result fields even when it reports an
    // ABI failure. Release unconditionally before converting the status so a
    // failed callback cannot strand a managed allocation.
    let copied = if status == ABI_OK {
        copy_debug_result(native)
    } else {
        match checked_status(status, "execute_debug") {
            Err(error) => Err(error),
            Ok(()) => unreachable!("only non-success debug callback statuses reach this branch"),
        }
    };
    // SAFETY: this exact generated release callback owns the returned result
    // allocation. It is called once for every callback invocation, including
    // zero/default results and non-success statuses.
    unsafe { release(handle, native) };
    copied
}

fn call_describe_debug(
    describe: NativeProductDescribeDebug,
    release: NativeProductReleaseDebugResult,
    handle: *mut c_void,
) -> Result<ProductHostDebugResult, CsharpProductRuntimeError> {
    let mut native = NativeProductDebugResult::default();
    // SAFETY: `native` is writable for this exact callback. Returned product
    // memory is copied before the matching release below.
    let status = unsafe { describe(handle, &mut native) };
    let copied = if status == ABI_OK {
        copy_debug_result(native)
    } else {
        match checked_status(status, "describe_debug") {
            Err(error) => Err(error),
            Ok(()) => {
                unreachable!("only non-success descriptor callback statuses reach this branch")
            }
        }
    };
    // SAFETY: the same generated result-release function owns descriptor
    // result allocations, including a result initialized before ABI failure.
    unsafe { release(handle, native) };
    let result = copied?;
    if !result.succeeded() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_DEBUG_DESCRIBE",
            "generated debug descriptor callback reported semantic failure",
        ));
    }
    Ok(result)
}

fn copy_debug_result(
    native: NativeProductDebugResult,
) -> Result<ProductHostDebugResult, CsharpProductRuntimeError> {
    let succeeded = match native.succeeded {
        0 => false,
        1 => true,
        value => {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_DEBUG_RESULT_STATUS",
                format!("generated debug callback returned invalid success flag {value}"),
            ));
        }
    };
    if native.message.len != 0 && native.message.bytes.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_DEBUG_RESULT_POINTER",
            "generated debug callback returned a null message with nonzero length",
        ));
    }
    // SAFETY: the generated callback guarantees this product-owned allocation
    // remains live until its matching release callback below. The length is
    // bounded before the slice is formed.
    let bytes = if native.message.len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(native.message.bytes, native.message.len) }
    };
    let message = std::str::from_utf8(bytes).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_DEBUG_RESULT_UTF8",
            format!("generated debug callback returned invalid UTF-8: {error}"),
        )
    })?;
    Ok(ProductHostDebugResult::new(succeeded, message.to_owned()))
}

fn call_complete_timeline(
    api: &LoadedProductApi,
    handle: *mut c_void,
    completion: &NativeProductTimelineCompletion,
) -> Result<bool, CsharpProductRuntimeError> {
    let mut accepted = 0u8;
    // SAFETY: all pointers in `completion` borrow local UTF-8/JSON buffers that
    // remain alive for this call; the generated C# bootstrap copies them.
    let status = unsafe { (api.complete_timeline)(handle, completion, &mut accepted) };
    if let Err(fallback) = checked_status(status, "complete_timeline") {
        return Err(read_product_call_error(api, handle).unwrap_or(fallback));
    }
    match accepted {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(CsharpProductRuntimeError::new(
            "CSHARP_TIMELINE_ACCEPTED",
            format!("C# product returned invalid timeline acceptance value {value}"),
        )),
    }
}

fn call_paused_intents(
    api: &LoadedProductApi,
    handle: *mut c_void,
    events: &[NativeInputEvent],
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: each event borrows owned storage that outlives this call; the
    // C# product copies anything it retains.
    let status = unsafe { (api.paused_intents)(handle, events.as_ptr(), events.len()) };
    if let Err(fallback) = checked_status(status, "paused_intents") {
        return Err(read_product_call_error(api, handle).unwrap_or(fallback));
    }
    Ok(())
}

fn observe_product_runtime(
    api: &LoadedProductApi,
    handle: *mut c_void,
    readout: RuntimeLifecycleReadout,
) {
    let facts = NativeProductRuntimeFacts {
        lifecycle_state: native_lifecycle_state(readout.state()),
        instance_id: readout.instance_id().value(),
        generation: readout.generation().value(),
        control_revision: readout.control_revision().value(),
    };
    // SAFETY: the generated observer borrows `facts` for this call only and
    // copies it into managed state. The product handle stays live until Drop.
    unsafe { (api.observe_runtime)(handle, &facts) };
}

fn native_utf8(value: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_bytes().as_ptr(),
        len: value.len(),
    }
}

fn native_optional_bytes(value: Option<&[u8]>) -> NativeByteSlice {
    value.map_or(
        NativeByteSlice {
            bytes: ptr::null(),
            len: 0,
        },
        |bytes| NativeByteSlice {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        },
    )
}

fn checked_status(status: i32, operation: &str) -> Result<(), CsharpProductRuntimeError> {
    if status != ABI_OK {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_CALL",
            format!("C# product {operation} returned status {status}"),
        ));
    }
    Ok(())
}

/// One bounded line for a faulted product call: the operation, the error
/// code and the first line of its detail. A product exception's first line is
/// the SDK's summary of its type, message and first product frame.
fn fault_line(operation: &str, failure: &CsharpProductRuntimeError) -> String {
    const MAXIMUM_DETAIL_CHARS: usize = 800;
    let first = failure.detail().lines().next().unwrap_or_default();
    let mut detail: String = first.chars().take(MAXIMUM_DETAIL_CHARS).collect();
    if detail.len() < first.len() {
        detail.push_str("...");
    }
    format!(
        "rusty: product {operation} faulted: {}: {detail}",
        failure.code()
    )
}

fn operation_name(operation: ProductHostOperationKind) -> &'static str {
    match operation {
        ProductHostOperationKind::Connect => "attach",
        ProductHostOperationKind::Start => "start",
        ProductHostOperationKind::Pause => "pause",
        ProductHostOperationKind::Resume => "resume",
        ProductHostOperationKind::Restart => "restart",
        ProductHostOperationKind::Shutdown => "shutdown",
        _ => "operation",
    }
}

#[derive(Debug)]
struct ContentFile {
    path: Vec<u8>,
    bytes: Arc<[u8]>,
}

/// Exact product content collected once before the native runtime and immutable
/// browser bundle are constructed. Renderer bytes stay inert until C# selects a
/// supported resource through the generated appearance API during `Create`.
pub struct CsharpProductContent {
    source: ProductSource,
    /// The content root within `source`.
    root: String,
    files: Vec<ContentFile>,
    appearance_catalog: CsharpAppearanceCatalog,
    bundles: csharp_engine_services::ProductContentBundles,
}

impl CsharpProductContent {
    /// Admits a loose content directory.
    pub fn admit(root: impl AsRef<Path>) -> Result<Self, CsharpProductRuntimeError> {
        let root = root.as_ref();
        let root_metadata = fs::symlink_metadata(root).map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_CONTENT_ROOT", error.to_string())
        })?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_ROOT",
                format!(
                    "content root must be a directory, not a symlink: {}",
                    root.display()
                ),
            ));
        }
        let source = ProductSource::open(root).map_err(content_error)?;
        Self::admit_from(&source, "")
    }

    /// Admits the content under `root` in a staged Product, loose or packed.
    /// Bundle members are left for `OpenBundle`; every other file is read now.
    pub fn admit_from(
        source: &ProductSource,
        root: &str,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let bundles = csharp_engine_services::ProductContentBundles::admit(source, root)
            .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CONTENT_BUNDLES", error))?;
        let mut files = Vec::new();
        for path in source.files(root).map_err(content_error)? {
            let relative = match root {
                "" => path.as_str(),
                root => &path[root.len() + 1..],
            };
            if bundles.owns_path(relative) {
                continue;
            }
            let bytes = source.read(&path).map_err(content_error)?;
            files.push(ContentFile {
                path: relative.as_bytes().to_vec(),
                bytes: Arc::from(bytes.as_ref()),
            });
        }
        let appearance_catalog = files
            .iter()
            .find(|file| file.path == b"runtime-appearances.json")
            .map(|file| file.bytes.as_ref());
        let appearance_catalog =
            csharp_engine_services::parse_runtime_appearance_catalog(appearance_catalog)?;
        Ok(Self {
            source: source.clone(),
            root: root.to_owned(),
            files,
            appearance_catalog,
            bundles,
        })
    }
}

fn content_error(error: product_container::Error) -> CsharpProductRuntimeError {
    let code = match error {
        product_container::Error::InvalidPath(_) => "CSHARP_CONTENT_PATH",
        product_container::Error::NotRegular(_) => "CSHARP_CONTENT_ENTRY",
        _ => "CSHARP_CONTENT_READ",
    };
    CsharpProductRuntimeError::new(code, error.to_string())
}

struct NativeInputOwned {
    kind: NativeInputEventKind,
    edge: NativeInputEdge,
    device: NativeInputDevice,
    channel: NativeInputChannel,
    axis: NativeInputAxis,
    keyboard: NativeKeyboardControl,
    pointer_button: NativePointerButton,
    controller_button: NativeControllerButton,
    controller_axis: NativeControllerAxis,
    clear_reason: NativeInputClearReason,
    value_kind: NativeInputValueKind,
    phase: NativeInputPhase,
    provenance: NativeInputProvenance,
    binding: NativeInputBinding,
    sequence: u64,
    x: f32,
    y: f32,
    has_position: bool,
    label: Vec<u8>,
    mapping_id: Vec<u8>,
    intent: Vec<u8>,
    context: Vec<u8>,
    payload_contract: Vec<u8>,
    payload_data: Vec<u8>,
}

impl NativeInputOwned {
    fn as_native(&self) -> NativeInputEvent {
        NativeInputEvent {
            kind: self.kind,
            edge: self.edge,
            device: self.device,
            channel: self.channel,
            axis: self.axis,
            keyboard: self.keyboard,
            pointer_button: self.pointer_button,
            controller_button: self.controller_button,
            controller_axis: self.controller_axis,
            clear_reason: self.clear_reason,
            value_kind: self.value_kind,
            phase: self.phase,
            provenance: self.provenance,
            binding: self.binding,
            sequence: NativeInputSequence {
                value: self.sequence,
            },
            x: self.x,
            y: self.y,
            has_position: self.has_position,
            label: self.label.as_ptr(),
            label_len: self.label.len(),
            mapping_id: self.mapping_id.as_ptr(),
            mapping_id_len: self.mapping_id.len(),
            intent: self.intent.as_ptr(),
            intent_len: self.intent.len(),
            context: self.context.as_ptr(),
            context_len: self.context.len(),
            payload_contract: self.payload_contract.as_ptr(),
            payload_contract_len: self.payload_contract.len(),
            payload_data: self.payload_data.as_ptr(),
            payload_data_len: self.payload_data.len(),
        }
    }
}

fn native_input_mapping(
    mapping: &RuntimeInputMapping,
    chord_storage: &mut Vec<Vec<NativeKeyboardControl>>,
) -> NativeInputMapping {
    let (context, context_len) = mapping
        .trigger()
        .context()
        .map_or((ptr::null(), 0), |value| {
            (value.as_str().as_bytes().as_ptr(), value.as_str().len())
        });
    let mut native = NativeInputMapping {
        id: mapping.id().as_bytes().as_ptr(),
        id_len: mapping.id().len(),
        intent: mapping.intent().as_bytes().as_ptr(),
        intent_len: mapping.intent().len(),
        trigger_kind: NativeInputTriggerKind::Key,
        edge: NativeInputEdge::None,
        axis: NativeInputAxis::None,
        keyboard: NativeKeyboardControl::None,
        pointer_button: NativePointerButton::None,
        controller_button: NativeControllerButton::None,
        controller_axis: NativeControllerAxis::None,
        chord: ptr::null(),
        chord_len: 0,
        context,
        context_len,
    };
    match mapping.trigger() {
        RuntimeInputTrigger::Key {
            code, edge, chord, ..
        } => {
            native.trigger_kind = NativeInputTriggerKind::Key;
            native.edge = configured_edge(*edge);
            native.keyboard = keyboard_control(*code);
            let converted = chord.iter().copied().map(keyboard_control).collect();
            chord_storage.push(converted);
            let stored = chord_storage
                .last()
                .expect("the just-pushed keyboard chord remains present");
            native.chord = stored.as_ptr();
            native.chord_len = stored.len();
        }
        RuntimeInputTrigger::PointerButton { button, edge, .. } => {
            native.trigger_kind = NativeInputTriggerKind::PointerButton;
            native.edge = configured_edge(*edge);
            native.pointer_button = pointer_button(*button);
        }
        RuntimeInputTrigger::PointerAxis { axis, .. } => {
            native.trigger_kind = NativeInputTriggerKind::PointerAxis;
            native.axis = input_axis(*axis);
        }
        RuntimeInputTrigger::Wheel { axis, .. } => {
            native.trigger_kind = NativeInputTriggerKind::Wheel;
            native.axis = input_axis(*axis);
        }
        RuntimeInputTrigger::ControllerButton { button, edge, .. } => {
            native.trigger_kind = NativeInputTriggerKind::ControllerButton;
            native.edge = configured_edge(*edge);
            native.controller_button = controller_button(*button);
        }
        RuntimeInputTrigger::ControllerButtonValue { button, .. } => {
            native.trigger_kind = NativeInputTriggerKind::ControllerButtonValue;
            native.controller_button = controller_button(*button);
        }
        RuntimeInputTrigger::ControllerAxis { axis, .. } => {
            native.trigger_kind = NativeInputTriggerKind::ControllerAxis;
            native.controller_axis = controller_axis(*axis);
        }
    }
    native
}

fn native_event(event: &RuntimeInputEvent) -> NativeInputOwned {
    match event {
        RuntimeInputEvent::Physical(physical) => {
            let mut native = input_owned(
                physical.sequence(),
                physical.runtime(),
                physical.context().as_str().as_bytes().to_vec(),
            );
            let (
                kind,
                edge,
                device,
                channel,
                axis,
                keyboard,
                pointer_button,
                controller_button,
                controller_axis,
                clear_reason,
                x,
                y,
                label,
            ) = match physical.fact() {
                runtime_input::RuntimeInputFact::Key { code, edge } => (
                    NativeInputEventKind::Key,
                    edge_value(*edge),
                    NativeInputDevice::Keyboard,
                    NativeInputChannel::Key,
                    NativeInputAxis::None,
                    keyboard_control(*code),
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    0.0,
                    0.0,
                    format!("{code:?}"),
                ),
                runtime_input::RuntimeInputFact::PointerButton {
                    button,
                    edge,
                    position,
                } => (
                    NativeInputEventKind::PointerButton,
                    edge_value(*edge),
                    NativeInputDevice::Pointer,
                    NativeInputChannel::Button,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    pointer_button(*button),
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    position.map_or(0.0, |at| at.x.value()),
                    position.map_or(0.0, |at| at.y.value()),
                    format!("{button:?}"),
                ),
                runtime_input::RuntimeInputFact::PointerPosition(at) => (
                    NativeInputEventKind::PointerPosition,
                    NativeInputEdge::None,
                    NativeInputDevice::Pointer,
                    NativeInputChannel::PointerPosition,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    at.x.value(),
                    at.y.value(),
                    String::new(),
                ),
                runtime_input::RuntimeInputFact::PointerDelta { x, y } => (
                    NativeInputEventKind::PointerDelta,
                    NativeInputEdge::None,
                    NativeInputDevice::Pointer,
                    NativeInputChannel::PointerDelta,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    x.value(),
                    y.value(),
                    String::new(),
                ),
                runtime_input::RuntimeInputFact::Wheel { x, y } => (
                    NativeInputEventKind::Wheel,
                    NativeInputEdge::None,
                    NativeInputDevice::Pointer,
                    NativeInputChannel::Wheel,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    x.value(),
                    y.value(),
                    String::new(),
                ),
                runtime_input::RuntimeInputFact::ControllerButton { button, edge } => (
                    NativeInputEventKind::ControllerButton,
                    edge_value(*edge),
                    NativeInputDevice::Controller,
                    NativeInputChannel::Button,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    controller_button(*button),
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    0.0,
                    0.0,
                    format!("{button:?}"),
                ),
                runtime_input::RuntimeInputFact::ControllerAxis { axis, value } => (
                    NativeInputEventKind::ControllerAxis,
                    NativeInputEdge::None,
                    NativeInputDevice::Controller,
                    NativeInputChannel::Axis,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    controller_axis(*axis),
                    NativeInputClearReason::None,
                    value.value(),
                    0.0,
                    format!("{axis:?}"),
                ),
                runtime_input::RuntimeInputFact::ControllerButtonValue { button, value } => (
                    NativeInputEventKind::ControllerButtonValue,
                    NativeInputEdge::None,
                    NativeInputDevice::Controller,
                    NativeInputChannel::Button,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    controller_button(*button),
                    NativeControllerAxis::None,
                    NativeInputClearReason::None,
                    value.value(),
                    0.0,
                    format!("{button:?}"),
                ),
                runtime_input::RuntimeInputFact::Clear { reason } => (
                    NativeInputEventKind::Clear,
                    NativeInputEdge::None,
                    NativeInputDevice::Runtime,
                    NativeInputChannel::Clear,
                    NativeInputAxis::None,
                    NativeKeyboardControl::None,
                    NativePointerButton::None,
                    NativeControllerButton::None,
                    NativeControllerAxis::None,
                    clear_reason(*reason),
                    0.0,
                    0.0,
                    format!("{reason:?}"),
                ),
            };
            native.kind = kind;
            native.edge = edge;
            native.device = device;
            native.channel = channel;
            native.axis = axis;
            native.keyboard = keyboard;
            native.pointer_button = pointer_button;
            native.controller_button = controller_button;
            native.controller_axis = controller_axis;
            native.clear_reason = clear_reason;
            native.x = x;
            native.y = y;
            native.has_position = matches!(
                physical.fact(),
                runtime_input::RuntimeInputFact::PointerPosition(_)
                    | runtime_input::RuntimeInputFact::PointerButton {
                        position: Some(_),
                        ..
                    }
            );
            native.provenance = NativeInputProvenance::Physical;
            native.label = label.into_bytes();
            native
        }
        RuntimeInputEvent::DirectIntent(intent) => {
            let (kind, value_kind, x, y, payload_contract, payload_data) = match intent.value() {
                RuntimeIntentValue::Digital { active } => (
                    NativeInputEventKind::DirectDigital,
                    NativeInputValueKind::Digital,
                    if active { 1.0 } else { 0.0 },
                    0.0,
                    Vec::new(),
                    Vec::new(),
                ),
                RuntimeIntentValue::Axis { value } => (
                    NativeInputEventKind::DirectAxis,
                    NativeInputValueKind::Axis,
                    value.value(),
                    0.0,
                    Vec::new(),
                    Vec::new(),
                ),
                RuntimeIntentValue::ProductPayload { payload } => (
                    NativeInputEventKind::DirectProductPayload,
                    NativeInputValueKind::ProductPayload,
                    0.0,
                    0.0,
                    payload.contract().as_bytes().to_vec(),
                    payload.bytes().to_vec(),
                ),
            };
            let mut native = input_owned(
                intent.sequence(),
                intent.runtime(),
                intent.context().as_str().as_bytes().to_vec(),
            );
            native.kind = kind;
            native.device = NativeInputDevice::Product;
            native.channel = NativeInputChannel::Intent;
            native.value_kind = value_kind;
            native.phase = NativeInputPhase::DirectUi;
            native.provenance = NativeInputProvenance::DirectUi;
            native.x = x;
            native.y = y;
            native.intent = intent.intent().as_bytes().to_vec();
            native.label = native.intent.clone();
            native.payload_contract = payload_contract;
            native.payload_data = payload_data;
            native
        }
    }
}

fn native_intent_event(
    envelope: &RuntimeIntentEnvelope,
    context: &InputContext,
) -> NativeInputOwned {
    let (mapping_id, provenance) = match envelope.provenance() {
        runtime_input::IntentProvenance::Physical { mapping_id } => {
            (mapping_id.as_str(), NativeInputProvenance::Physical)
        }
        runtime_input::IntentProvenance::DirectUi => ("", NativeInputProvenance::DirectUi),
    };
    let (kind, value_kind, x, payload_contract, payload_data) = match envelope.value() {
        RuntimeIntentValue::Digital { active } => (
            match provenance {
                NativeInputProvenance::DirectUi => NativeInputEventKind::DirectDigital,
                _ => NativeInputEventKind::MappedDigital,
            },
            NativeInputValueKind::Digital,
            if active { 1.0 } else { 0.0 },
            Vec::new(),
            Vec::new(),
        ),
        RuntimeIntentValue::Axis { value } => (
            match provenance {
                NativeInputProvenance::DirectUi => NativeInputEventKind::DirectAxis,
                _ => NativeInputEventKind::MappedAxis,
            },
            NativeInputValueKind::Axis,
            value.value(),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeIntentValue::ProductPayload { payload } => (
            match provenance {
                NativeInputProvenance::DirectUi => NativeInputEventKind::DirectProductPayload,
                _ => NativeInputEventKind::MappedProductPayload,
            },
            NativeInputValueKind::ProductPayload,
            0.0,
            payload.contract().as_bytes().to_vec(),
            payload.bytes().to_vec(),
        ),
    };
    let mut native = input_owned(
        envelope.sequence(),
        envelope.runtime(),
        context.as_str().as_bytes().to_vec(),
    );
    native.kind = kind;
    native.device = NativeInputDevice::Product;
    native.channel = NativeInputChannel::Intent;
    native.value_kind = value_kind;
    native.phase = native_input_phase(envelope.phase());
    native.edge = native_input_edge(envelope.phase());
    native.provenance = provenance;
    native.x = x;
    native.mapping_id = mapping_id.as_bytes().to_vec();
    native.intent = envelope.intent().as_bytes().to_vec();
    native.label = if provenance == NativeInputProvenance::DirectUi {
        native.intent.clone()
    } else {
        native.mapping_id.clone()
    };
    native.payload_contract = payload_contract;
    native.payload_data = payload_data;
    native
}

fn input_owned(
    sequence: u64,
    binding: runtime_input::RuntimeInputBinding,
    context: Vec<u8>,
) -> NativeInputOwned {
    NativeInputOwned {
        kind: NativeInputEventKind::Clear,
        edge: NativeInputEdge::None,
        device: NativeInputDevice::Runtime,
        channel: NativeInputChannel::None,
        axis: NativeInputAxis::None,
        keyboard: NativeKeyboardControl::None,
        pointer_button: NativePointerButton::None,
        controller_button: NativeControllerButton::None,
        controller_axis: NativeControllerAxis::None,
        clear_reason: NativeInputClearReason::None,
        value_kind: NativeInputValueKind::None,
        phase: NativeInputPhase::None,
        provenance: NativeInputProvenance::None,
        binding: NativeInputBinding {
            instance_id: binding.instance_id().value(),
            generation: binding.generation().value(),
            control_revision: binding.control_revision().value(),
        },
        sequence,
        x: 0.0,
        y: 0.0,
        has_position: false,
        label: Vec::new(),
        mapping_id: Vec::new(),
        intent: Vec::new(),
        context,
        payload_contract: Vec::new(),
        payload_data: Vec::new(),
    }
}

fn edge_value(edge: runtime_input::PhysicalEdge) -> NativeInputEdge {
    match edge {
        runtime_input::PhysicalEdge::Pressed => NativeInputEdge::Pressed,
        runtime_input::PhysicalEdge::Released => NativeInputEdge::Released,
    }
}

fn native_input_edge(phase: runtime_input::IntentPhase) -> NativeInputEdge {
    match phase {
        runtime_input::IntentPhase::Held => NativeInputEdge::Held,
        runtime_input::IntentPhase::Pressed => NativeInputEdge::Pressed,
        runtime_input::IntentPhase::Released => NativeInputEdge::Released,
        runtime_input::IntentPhase::Axis | runtime_input::IntentPhase::DirectUi => {
            NativeInputEdge::None
        }
    }
}

fn native_input_phase(phase: runtime_input::IntentPhase) -> NativeInputPhase {
    match phase {
        runtime_input::IntentPhase::Held => NativeInputPhase::Held,
        runtime_input::IntentPhase::Pressed => NativeInputPhase::Pressed,
        runtime_input::IntentPhase::Released => NativeInputPhase::Released,
        runtime_input::IntentPhase::Axis => NativeInputPhase::Axis,
        runtime_input::IntentPhase::DirectUi => NativeInputPhase::DirectUi,
    }
}

fn configured_edge(edge: InputEdge) -> NativeInputEdge {
    match edge {
        InputEdge::Held => NativeInputEdge::Held,
        InputEdge::Pressed => NativeInputEdge::Pressed,
        InputEdge::Released => NativeInputEdge::Released,
    }
}

fn input_axis(axis: InputAxis) -> NativeInputAxis {
    match axis {
        InputAxis::X => NativeInputAxis::X,
        InputAxis::Y => NativeInputAxis::Y,
    }
}

fn keyboard_control(value: runtime_input_model::KeyboardControl) -> NativeKeyboardControl {
    match value {
        runtime_input_model::KeyboardControl::KeyA => NativeKeyboardControl::KeyA,
        runtime_input_model::KeyboardControl::KeyB => NativeKeyboardControl::KeyB,
        runtime_input_model::KeyboardControl::KeyC => NativeKeyboardControl::KeyC,
        runtime_input_model::KeyboardControl::KeyD => NativeKeyboardControl::KeyD,
        runtime_input_model::KeyboardControl::KeyE => NativeKeyboardControl::KeyE,
        runtime_input_model::KeyboardControl::KeyF => NativeKeyboardControl::KeyF,
        runtime_input_model::KeyboardControl::KeyG => NativeKeyboardControl::KeyG,
        runtime_input_model::KeyboardControl::KeyH => NativeKeyboardControl::KeyH,
        runtime_input_model::KeyboardControl::KeyI => NativeKeyboardControl::KeyI,
        runtime_input_model::KeyboardControl::KeyJ => NativeKeyboardControl::KeyJ,
        runtime_input_model::KeyboardControl::KeyK => NativeKeyboardControl::KeyK,
        runtime_input_model::KeyboardControl::KeyL => NativeKeyboardControl::KeyL,
        runtime_input_model::KeyboardControl::KeyM => NativeKeyboardControl::KeyM,
        runtime_input_model::KeyboardControl::KeyN => NativeKeyboardControl::KeyN,
        runtime_input_model::KeyboardControl::KeyO => NativeKeyboardControl::KeyO,
        runtime_input_model::KeyboardControl::KeyP => NativeKeyboardControl::KeyP,
        runtime_input_model::KeyboardControl::KeyQ => NativeKeyboardControl::KeyQ,
        runtime_input_model::KeyboardControl::KeyR => NativeKeyboardControl::KeyR,
        runtime_input_model::KeyboardControl::KeyS => NativeKeyboardControl::KeyS,
        runtime_input_model::KeyboardControl::KeyT => NativeKeyboardControl::KeyT,
        runtime_input_model::KeyboardControl::KeyU => NativeKeyboardControl::KeyU,
        runtime_input_model::KeyboardControl::KeyV => NativeKeyboardControl::KeyV,
        runtime_input_model::KeyboardControl::KeyW => NativeKeyboardControl::KeyW,
        runtime_input_model::KeyboardControl::KeyX => NativeKeyboardControl::KeyX,
        runtime_input_model::KeyboardControl::KeyY => NativeKeyboardControl::KeyY,
        runtime_input_model::KeyboardControl::KeyZ => NativeKeyboardControl::KeyZ,
        runtime_input_model::KeyboardControl::Digit0 => NativeKeyboardControl::Digit0,
        runtime_input_model::KeyboardControl::Digit1 => NativeKeyboardControl::Digit1,
        runtime_input_model::KeyboardControl::Digit2 => NativeKeyboardControl::Digit2,
        runtime_input_model::KeyboardControl::Digit3 => NativeKeyboardControl::Digit3,
        runtime_input_model::KeyboardControl::Digit4 => NativeKeyboardControl::Digit4,
        runtime_input_model::KeyboardControl::Digit5 => NativeKeyboardControl::Digit5,
        runtime_input_model::KeyboardControl::Digit6 => NativeKeyboardControl::Digit6,
        runtime_input_model::KeyboardControl::Digit7 => NativeKeyboardControl::Digit7,
        runtime_input_model::KeyboardControl::Digit8 => NativeKeyboardControl::Digit8,
        runtime_input_model::KeyboardControl::Digit9 => NativeKeyboardControl::Digit9,
        runtime_input_model::KeyboardControl::Space => NativeKeyboardControl::Space,
        runtime_input_model::KeyboardControl::Enter => NativeKeyboardControl::Enter,
        runtime_input_model::KeyboardControl::Escape => NativeKeyboardControl::Escape,
        runtime_input_model::KeyboardControl::ShiftLeft => NativeKeyboardControl::ShiftLeft,
        runtime_input_model::KeyboardControl::ShiftRight => NativeKeyboardControl::ShiftRight,
        runtime_input_model::KeyboardControl::ControlLeft => NativeKeyboardControl::ControlLeft,
        runtime_input_model::KeyboardControl::ControlRight => NativeKeyboardControl::ControlRight,
        runtime_input_model::KeyboardControl::AltLeft => NativeKeyboardControl::AltLeft,
        runtime_input_model::KeyboardControl::AltRight => NativeKeyboardControl::AltRight,
        runtime_input_model::KeyboardControl::ArrowUp => NativeKeyboardControl::ArrowUp,
        runtime_input_model::KeyboardControl::ArrowDown => NativeKeyboardControl::ArrowDown,
        runtime_input_model::KeyboardControl::ArrowLeft => NativeKeyboardControl::ArrowLeft,
        runtime_input_model::KeyboardControl::ArrowRight => NativeKeyboardControl::ArrowRight,
    }
}

fn pointer_button(value: runtime_input_model::PointerButton) -> NativePointerButton {
    match value {
        runtime_input_model::PointerButton::Primary => NativePointerButton::Primary,
        runtime_input_model::PointerButton::Secondary => NativePointerButton::Secondary,
        runtime_input_model::PointerButton::Middle => NativePointerButton::Middle,
    }
}

fn controller_button(value: runtime_input_model::ControllerButton) -> NativeControllerButton {
    match value {
        runtime_input_model::ControllerButton::Button0 => NativeControllerButton::Button0,
        runtime_input_model::ControllerButton::Button1 => NativeControllerButton::Button1,
        runtime_input_model::ControllerButton::Button2 => NativeControllerButton::Button2,
        runtime_input_model::ControllerButton::Button3 => NativeControllerButton::Button3,
        runtime_input_model::ControllerButton::Button4 => NativeControllerButton::Button4,
        runtime_input_model::ControllerButton::Button5 => NativeControllerButton::Button5,
        runtime_input_model::ControllerButton::Button6 => NativeControllerButton::Button6,
        runtime_input_model::ControllerButton::Button7 => NativeControllerButton::Button7,
        runtime_input_model::ControllerButton::Button8 => NativeControllerButton::Button8,
        runtime_input_model::ControllerButton::Button9 => NativeControllerButton::Button9,
        runtime_input_model::ControllerButton::Button10 => NativeControllerButton::Button10,
        runtime_input_model::ControllerButton::Button11 => NativeControllerButton::Button11,
        runtime_input_model::ControllerButton::Button12 => NativeControllerButton::Button12,
        runtime_input_model::ControllerButton::Button13 => NativeControllerButton::Button13,
        runtime_input_model::ControllerButton::Button14 => NativeControllerButton::Button14,
        runtime_input_model::ControllerButton::Button15 => NativeControllerButton::Button15,
    }
}

fn controller_axis(value: runtime_input_model::ControllerAxis) -> NativeControllerAxis {
    match value {
        runtime_input_model::ControllerAxis::Axis0 => NativeControllerAxis::Axis0,
        runtime_input_model::ControllerAxis::Axis1 => NativeControllerAxis::Axis1,
        runtime_input_model::ControllerAxis::Axis2 => NativeControllerAxis::Axis2,
        runtime_input_model::ControllerAxis::Axis3 => NativeControllerAxis::Axis3,
    }
}

fn clear_reason(value: runtime_input::InputClearReason) -> NativeInputClearReason {
    match value {
        runtime_input::InputClearReason::FocusLoss => NativeInputClearReason::FocusLoss,
        runtime_input::InputClearReason::InteractionModeLoss => {
            NativeInputClearReason::InteractionModeLoss
        }
        runtime_input::InputClearReason::PointerLockLoss => NativeInputClearReason::PointerLockLoss,
        runtime_input::InputClearReason::Restart => NativeInputClearReason::Restart,
        runtime_input::InputClearReason::ControlRevisionChange => {
            NativeInputClearReason::ControlRevisionChange
        }
        runtime_input::InputClearReason::Dispose => NativeInputClearReason::Dispose,
        runtime_input::InputClearReason::IngressOverflow => NativeInputClearReason::IngressOverflow,
    }
}

/// A call's frame with no operations and no publication revision changes
/// nothing a renderer holds. Baselines are built separately and keep theirs.
fn is_empty_frame(output: &RuntimePublication) -> bool {
    match output {
        RuntimePublication::Frame(frame) => frame.ops.is_empty() && frame.publication.is_none(),
        RuntimePublication::Presentation(frame) => {
            frame.ops.is_empty() && frame.publication.is_none()
        }
        _ => false,
    }
}

fn publication_error(error: RuntimePublicationError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new("RUNTIME_PUBLICATION", error.to_string())
}

fn service_outputs(
    output: CsharpEngineCallOutput,
) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
    let mut outputs = Vec::new();
    for appearance in output.appearance {
        match appearance {
            CsharpAppearanceCallOutput::Frame(frame) => {
                outputs.push(RuntimePublication::Frame(frame));
            }
            CsharpAppearanceCallOutput::Presentation(frame) => {
                outputs.push(RuntimePublication::Presentation(frame));
            }
        }
    }
    for frame in output.frames {
        outputs.push(RuntimePublication::Frame(frame));
    }
    if let Some(composition) = output.view_composition {
        outputs.push(RuntimePublication::ViewComposition(composition));
    }
    for projection in output.ui {
        outputs.push(RuntimePublication::UiProjection(projection));
    }
    for frame in output.presentation {
        outputs.push(RuntimePublication::Presentation(frame));
    }
    Ok(outputs)
}

/// A finished call's publications, after its settled output jobs go to the
/// executor.
fn call_outputs(
    executor: &render_output::OutputExecutor,
    mut output: csharp_engine_services::CsharpEngineCallOutput,
) -> Result<Vec<RuntimePublication>, CsharpProductRuntimeError> {
    executor.submit(std::mem::take(&mut output.render_output));
    service_outputs(output)
}

fn assert_ui_projection_binding(
    outputs: &[RuntimePublication],
    expected: RuntimeInputBinding,
) -> Result<usize, CsharpProductRuntimeError> {
    let mut count = 0;
    for output in outputs {
        let RuntimePublication::UiProjection(envelope) = output else {
            continue;
        };
        count += 1;
        let runtime = envelope.runtime();
        if runtime.instance_id() != expected.instance_id()
            || runtime.generation() != expected.generation()
            || runtime.control_revision() != expected.control_revision()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_UI_BINDING",
                "UI projection runtime identity did not match the admitted runtime binding",
            ));
        }
    }
    if count != 0 {
        Ok(count)
    } else {
        Err(CsharpProductRuntimeError::new(
            "CSHARP_EXERCISE_UI_BINDING",
            "admitted product update did not publish a UI projection",
        ))
    }
}

fn complete_voxel_baseline(
    outputs: &[RuntimePublication],
) -> Result<serde_json::Value, CsharpProductRuntimeError> {
    const REQUIRED: [&str; 3] = ["defineMaterial", "create", "replaceMeshPayload"];
    let mut observed_frames = Vec::new();
    for output in outputs {
        let RuntimePublication::Frame(frame) = output else {
            continue;
        };
        let frame = serde_json::to_value(frame).map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_EXERCISE_ATTACH", error.to_string())
        })?;
        let operations = frame
            .get("ops")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_ATTACH",
                    "fresh voxel attachment frame did not expose typed operations",
                )
            })?;
        let observed: std::collections::BTreeSet<_> = operations
            .iter()
            .filter_map(|operation| operation.get("op").and_then(serde_json::Value::as_str))
            .collect();
        let missing: Vec<_> = REQUIRED
            .iter()
            .copied()
            .filter(|required| !observed.contains(required))
            .collect();
        if missing.is_empty() {
            return Ok(frame);
        }
        observed_frames.push(format!(
            "frame {}: observed [{}], missing [{}]",
            observed_frames.len() + 1,
            observed.into_iter().collect::<Vec<_>>().join(", "),
            missing.join(", ")
        ));
    }
    Err(CsharpProductRuntimeError::new(
        "CSHARP_EXERCISE_ATTACH",
        format!(
            "fresh browser attachment did not publish a complete retained voxel baseline; {}; \
             --exercise requires defineMaterial, create and replaceMeshPayload in one frame. \
             This is an Engine fixture check, not a general product health check. \
             The fixture commits a nonempty Voxel scene with a retained VoxelScenePresentation \
             projection before attachment; product metadata alone does not create it. \
             Products without voxel content should launch without --exercise. See docs/csharp-product-project.md#host-exercise-contract",
            if observed_frames.is_empty() {
                "no frame publications observed; missing [defineMaterial, create, replaceMeshPayload]".to_owned()
            } else {
                observed_frames.join("; ")
            }
        ),
    ))
}

fn host_runtime_error(error: product_host::ProductHostError) -> ProductHostRuntimeError {
    ProductHostRuntimeError::new(error.code(), error.detail().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A publication's kind with its graphics or presentation frame, as JSON
    /// to inspect.
    fn publication_value(output: &RuntimePublication) -> serde_json::Value {
        let kind = publication_kind(output);
        match output {
            RuntimePublication::Frame(frame) => serde_json::json!({ "kind": kind, "frame": frame }),
            RuntimePublication::Presentation(frame) => {
                serde_json::json!({ "kind": kind, "frame": frame })
            }
            _ => serde_json::json!({ "kind": kind }),
        }
    }

    fn publication_kind(output: &RuntimePublication) -> &'static str {
        match output {
            RuntimePublication::Binding { .. } => "binding",
            RuntimePublication::CompleteBaseline { .. } => "complete-baseline",
            RuntimePublication::Frame(_) => "frame",
            RuntimePublication::ViewComposition(_) => "view-composition",
            RuntimePublication::Presentation(_) => "presentation",
            RuntimePublication::UiProjection(_) => "ui-projection",
        }
    }

    use std::sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering},
        Mutex, PoisonError,
    };

    static CONTENT_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
    static DEBUG_FIXTURE_GATE: Mutex<()> = Mutex::new(());
    static DEBUG_RELEASES: AtomicUsize = AtomicUsize::new(0);
    static DROP_FIXTURE_GATE: Mutex<()> = Mutex::new(());
    static DROP_CALLBACK_STATUS: AtomicI32 = AtomicI32::new(ABI_OK);
    static UPDATE_CALLBACK_STATUS: AtomicI32 = AtomicI32::new(ABI_OK);
    static UPDATE_CALLBACK_CALLS: AtomicUsize = AtomicUsize::new(0);
    static UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC: AtomicBool = AtomicBool::new(false);
    static UPDATE_CALLBACK_DIAGNOSTIC_STATUS: AtomicI32 = AtomicI32::new(0);
    static UPDATE_CALLBACK_PUBLISH_UI: AtomicBool = AtomicBool::new(false);
    static FIXTURE_UI_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static FIXTURE_UI_OPEN: AtomicUsize = AtomicUsize::new(0);
    static FIXTURE_UI_PUBLISH: AtomicUsize = AtomicUsize::new(0);
    /// The gameplay time table from the last fixture create.
    struct FixtureGameplayTime(NativeGameplayTimeApi);
    // SAFETY: tests serialize fixture products with DROP_FIXTURE_GATE and use
    // the table only inside that product's callbacks.
    unsafe impl Send for FixtureGameplayTime {}
    static FIXTURE_GAMEPLAY_TIME: Mutex<Option<FixtureGameplayTime>> = Mutex::new(None);
    static VOXEL_FAILURE_ENABLED: AtomicBool = AtomicBool::new(false);
    static VOXEL_FAILURE_SESSION: AtomicU64 = AtomicU64::new(0);
    static VOXEL_FAILURE_PRESENTATION: AtomicU64 = AtomicU64::new(0);
    static VOXEL_FAILURE_SPATIAL_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_DESTROY_SESSION: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_DESTROY_SESSION_ENABLED: AtomicBool = AtomicBool::new(false);
    static VOXEL_FAILURE_DESTROY_SESSION_STATUS: AtomicI32 = AtomicI32::new(0);
    static VOXEL_FAILURE_VOXEL_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_APPLY_EDITS: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_PRESENTATION_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_REFRESH_SCENE: AtomicUsize = AtomicUsize::new(0);
    static VOXEL_FAILURE_APPLY_STATUS: AtomicI32 = AtomicI32::new(0);
    static VOXEL_FAILURE_REFRESH_STATUS: AtomicI32 = AtomicI32::new(0);
    static FIXTURE_DIAGNOSTICS_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static FIXTURE_DIAGNOSTICS_PUBLISH: AtomicUsize = AtomicUsize::new(0);
    static AUDIO_DISPOSE_FIXTURE_ENABLED: AtomicBool = AtomicBool::new(false);
    static AUDIO_DISPOSE_FIXTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
    static AUDIO_DISPOSE_AUDIO_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static AUDIO_DISPOSE_AUDIO_DESTROY: AtomicUsize = AtomicUsize::new(0);
    static AUDIO_DISPOSE_CLIP: AtomicU64 = AtomicU64::new(0);
    static AUDIO_DISPOSE_DESTROY_STATUS: AtomicI32 = AtomicI32::new(-1);
    static DROP_EVENTS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static PRODUCT_ERROR_RELEASES: AtomicUsize = AtomicUsize::new(0);
    static DIRECT_INPUT_FIXTURE_GATE: Mutex<()> = Mutex::new(());
    static DIRECT_INPUT_CALLBACK_EVENTS: Mutex<Vec<Vec<DirectInputCallbackEvent>>> =
        Mutex::new(Vec::new());
    static REMAPPING_CALLBACK_CONTEXT: AtomicUsize = AtomicUsize::new(0);
    static REMAPPING_CALLBACK_REPLACE: AtomicUsize = AtomicUsize::new(0);
    static REMAPPING_CALLBACK_STAGE: AtomicBool = AtomicBool::new(false);
    static REMAPPING_CALLBACK_STATUS: AtomicI32 = AtomicI32::new(0);
    static REMAPPING_CALLBACK_OUTCOME: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn renderer_debug_owns_only_exact_engine_commands() {
        assert_eq!(
            renderer_debug_command("engine.renderer"),
            Some(RendererDebugCommand::Read)
        );
        assert_eq!(
            renderer_debug_command(" engine.renderer.toggle "),
            Some(RendererDebugCommand::Toggle)
        );
        assert_eq!(renderer_debug_command("engine.renderer.product"), None);
    }

    #[test]
    fn renderer_debug_commands_publish_widget_state_without_a_product_debug_callback() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("renderer-debug-commands");
        let (unavailable, _) = runtime
            .execute_debug("engine.renderer.presentation")
            .expect("presentation query without a renderer completes")
            .into_parts();
        assert!(unavailable.succeeded());
        let unavailable: serde_json::Value = serde_json::from_str(unavailable.message()).unwrap();
        assert_eq!(unavailable["available"], false);
        let (read, _) = runtime
            .execute_debug("engine.renderer")
            .expect("read without a renderer completes")
            .into_parts();
        assert!(
            !read.succeeded(),
            "a read with no renderer is a failed observation"
        );

        let (shown, _) = runtime
            .execute_debug("engine.renderer.show")
            .expect("Engine show command completes")
            .into_parts();
        assert!(shown.succeeded());
        let shown: serde_json::Value =
            serde_json::from_str(shown.message()).expect("show readout is JSON");
        assert_eq!(shown["widget"]["visible"], true);
        assert_eq!(shown["available"], false);

        let (hidden, _) = runtime
            .execute_debug("engine.renderer.hide")
            .expect("Engine hide command completes")
            .into_parts();
        let hidden: serde_json::Value =
            serde_json::from_str(hidden.message()).expect("hide readout is JSON");
        assert_eq!(hidden["widget"]["visible"], false);

        let (toggled, _) = runtime
            .execute_debug("engine.renderer.toggle")
            .expect("Engine toggle command completes")
            .into_parts();
        let toggled: serde_json::Value =
            serde_json::from_str(toggled.message()).expect("toggle readout is JSON");
        assert_eq!(toggled["widget"]["visible"], true);
        let (status, _) = runtime
            .execute_debug("engine.renderer.status")
            .expect("Engine status command completes")
            .into_parts();
        let status: serde_json::Value =
            serde_json::from_str(status.message()).expect("status readout is JSON");
        assert_eq!(status["widget"]["visible"], true);

        let (catalog, _) = runtime
            .describe_debug()
            .expect("catalog completes")
            .into_parts();
        let catalog = serde_json::to_value(catalog).expect("catalog serializes");
        assert!(catalog["commands"]
            .as_array()
            .expect("catalog commands")
            .iter()
            .any(|command| command["name"] == "engine.renderer.status"));
        drop(runtime);
        fs::remove_dir_all(root).expect("remove renderer debug fixture content");
    }

    #[test]
    fn the_scene_snapshot_command_writes_the_committed_scene_without_a_renderer() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("scene-snapshot");
        let (usage, _) = runtime
            .execute_debug("engine.renderer.snapshot")
            .expect("a snapshot without a path completes")
            .into_parts();
        assert!(!usage.succeeded() && usage.message().contains("<path>"));
        let path = root.join("scene.rscene");
        let (written, _) = runtime
            .execute_debug(&format!("engine.renderer.snapshot {}", path.display()))
            .expect("snapshot completes")
            .into_parts();
        assert!(written.succeeded(), "{}", written.message());
        let report: serde_json::Value = serde_json::from_str(written.message()).unwrap();
        assert_eq!(report["path"], path.display().to_string());
        let snapshot = scene_snapshot::SceneSnapshot::open(&path).expect("snapshot reads back");
        assert_eq!(
            snapshot.metadata.host.abi_fingerprint,
            product_host_runtime_identity().fingerprint_hex()
        );
        assert!(snapshot
            .changes
            .iter()
            .any(|change| matches!(change, scene_snapshot::SceneSnapshotChange::Frame(_))));
        assert!(snapshot.changes.iter().any(|change| matches!(
            change,
            scene_snapshot::SceneSnapshotChange::ViewComposition(_)
        )));
        let (catalog, _) = runtime.describe_debug().expect("catalog").into_parts();
        let catalog = serde_json::to_value(catalog).unwrap();
        assert!(catalog["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["name"] == "engine.renderer.snapshot"));
        drop(runtime);
        fs::remove_dir_all(root).expect("remove snapshot fixture content");
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DirectInputCallbackEvent {
        kind: NativeInputEventKind,
        provenance: NativeInputProvenance,
        phase: NativeInputPhase,
        sequence: u64,
        intent: Vec<u8>,
        payload_contract: Vec<u8>,
        payload_data: Vec<u8>,
    }

    unsafe fn copy_callback_bytes(bytes: *const u8, len: usize) -> Vec<u8> {
        if len == 0 {
            return Vec::new();
        }
        // SAFETY: the generated callback borrows every input slice for this
        // call, and this fixture copies the requested non-empty range before
        // returning.
        unsafe { std::slice::from_raw_parts(bytes, len).to_vec() }
    }

    unsafe extern "C" fn direct_input_fixture_update(
        _handle: *mut c_void,
        args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        // SAFETY: the runtime supplies valid callback arguments for this
        // immediate generated-boundary fixture invocation.
        let args = unsafe { args.as_ref().expect("direct-input update arguments") };
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            // SAFETY: `events` is borrowed for this callback with the declared
            // count.
            .push(unsafe { capture_callback_events(args.events, args.event_count) });
        // SAFETY: the fixture owns the callback result pointer.
        unsafe { *result = NativeProductUpdateResult::None };
        ABI_OK
    }

    static PAUSED_INTENT_CALLBACK_EVENTS: Mutex<Vec<Vec<DirectInputCallbackEvent>>> =
        Mutex::new(Vec::new());
    static PAUSED_INTENT_CALLBACK_STATUS: AtomicI32 = AtomicI32::new(ABI_OK);

    unsafe extern "C" fn paused_intent_fixture(
        _handle: *mut c_void,
        events: *const NativeInputEvent,
        count: usize,
    ) -> i32 {
        PAUSED_INTENT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            // SAFETY: `events` is borrowed for this callback with the declared
            // count.
            .push(unsafe { capture_callback_events(events, count) });
        PAUSED_INTENT_CALLBACK_STATUS.load(Ordering::SeqCst)
    }

    /// # Safety
    /// `events` points to `count` events whose byte slices stay valid for the
    /// call; the fixture copies them before returning.
    unsafe fn capture_callback_events(
        events: *const NativeInputEvent,
        count: usize,
    ) -> Vec<DirectInputCallbackEvent> {
        // SAFETY: guaranteed by the caller.
        let events = unsafe { std::slice::from_raw_parts(events, count) };
        events
            .iter()
            .map(|event| DirectInputCallbackEvent {
                kind: event.kind,
                provenance: event.provenance,
                phase: event.phase,
                sequence: event.sequence.value,
                // SAFETY: these byte slices remain valid for this callback.
                intent: unsafe { copy_callback_bytes(event.intent, event.intent_len) },
                // SAFETY: these byte slices remain valid for this callback.
                payload_contract: unsafe {
                    copy_callback_bytes(event.payload_contract, event.payload_contract_len)
                },
                // SAFETY: these byte slices remain valid for this callback.
                payload_data: unsafe {
                    copy_callback_bytes(event.payload_data, event.payload_data_len)
                },
            })
            .collect()
    }

    unsafe extern "C" fn remapping_callback_fixture_create(
        args: *const NativeProductCreateArgs,
        handle: *mut *mut c_void,
        error: *mut NativeProductCallError,
    ) -> i32 {
        let status = unsafe { drop_fixture_create(args, handle, error) };
        if status == ABI_OK {
            let input = unsafe { (*args).engine.input };
            REMAPPING_CALLBACK_CONTEXT.store(input.context as usize, Ordering::SeqCst);
            REMAPPING_CALLBACK_REPLACE
                .store(input.replace_physical_mappings as usize, Ordering::SeqCst);
        }
        status
    }

    unsafe extern "C" fn remapping_callback_fixture_update(
        handle: *mut c_void,
        args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        if REMAPPING_CALLBACK_STAGE.swap(false, Ordering::SeqCst) {
            let mapping = NativeInputMapping {
                id: b"attack-f".as_ptr(),
                id_len: b"attack-f".len(),
                intent: b"fixture.attack".as_ptr(),
                intent_len: b"fixture.attack".len(),
                trigger_kind: NativeInputTriggerKind::Key,
                edge: NativeInputEdge::Pressed,
                axis: NativeInputAxis::None,
                keyboard: NativeKeyboardControl::KeyF,
                pointer_button: NativePointerButton::None,
                controller_button: NativeControllerButton::None,
                controller_axis: NativeControllerAxis::None,
                chord: ptr::null(),
                chord_len: 0,
                context: ptr::null(),
                context_len: 0,
            };
            let replace: NativeReplaceInputMappings =
                unsafe { std::mem::transmute(REMAPPING_CALLBACK_REPLACE.load(Ordering::SeqCst)) };
            let mut outcome = NativeInputMappingReplacementOutcome::Unavailable;
            let status = unsafe {
                replace(
                    REMAPPING_CALLBACK_CONTEXT.load(Ordering::SeqCst) as *mut c_void,
                    &mapping,
                    1,
                    &mut outcome,
                    std::ptr::null_mut(),
                )
            };
            REMAPPING_CALLBACK_STATUS.store(status, Ordering::SeqCst);
            REMAPPING_CALLBACK_OUTCOME.store(outcome as usize, Ordering::SeqCst);
        }
        unsafe { direct_input_fixture_update(handle, args, result) }
    }

    fn record_drop_event(event: &'static str) {
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event);
    }

    fn fixture_utf8(bytes: &'static [u8]) -> NativeUtf8Slice {
        NativeUtf8Slice {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        }
    }

    unsafe extern "C" fn drop_fixture_create(
        args: *const NativeProductCreateArgs,
        handle: *mut *mut c_void,
        _error: *mut NativeProductCallError,
    ) -> i32 {
        // SAFETY: product creation receives the live Engine service table;
        // this fixture copies only the one diagnostics function/context needed
        // by its immediate update callback.
        let diagnostics = unsafe { (*args).engine.diagnostics };
        FIXTURE_DIAGNOSTICS_CONTEXT.store(diagnostics.context as usize, Ordering::SeqCst);
        FIXTURE_DIAGNOSTICS_PUBLISH.store(diagnostics.publish as usize, Ordering::SeqCst);
        *FIXTURE_GAMEPLAY_TIME
            .lock()
            .unwrap_or_else(PoisonError::into_inner) =
            Some(FixtureGameplayTime(unsafe { (*args).engine.gameplay_time }));
        let ui = unsafe { (*args).engine.ui };
        FIXTURE_UI_CONTEXT.store(ui.context as usize, Ordering::SeqCst);
        FIXTURE_UI_OPEN.store(ui.open_stream as usize, Ordering::SeqCst);
        FIXTURE_UI_PUBLISH.store(ui.publish_projection as usize, Ordering::SeqCst);
        let audio = unsafe { (*args).engine.audio };
        AUDIO_DISPOSE_AUDIO_CONTEXT.store(audio.context as usize, Ordering::SeqCst);
        AUDIO_DISPOSE_AUDIO_DESTROY.store(audio.destroy_clip as usize, Ordering::SeqCst);
        if AUDIO_DISPOSE_FIXTURE_ENABLED.swap(false, Ordering::SeqCst) {
            let path = b"content/audio/dispose.wav";
            let request = NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            };
            let open: NativeOpenAudioClip =
                unsafe { std::mem::transmute(audio.open_clip as usize) };
            let mut clip = NativeAudioClipHandle::default();
            let mut operation_error = unsafe { std::mem::zeroed() };
            let open_status =
                unsafe { open(audio.context, &request, &mut clip, &mut operation_error) };
            if open_status != ABI_OK {
                return open_status;
            }
            let signal_id = b"dispose-one-shot";
            let emit: NativeEmitAudio = unsafe { std::mem::transmute(audio.emit as usize) };
            let mut signal = NativeAudioSignalHandle::default();
            let mut operation_error = unsafe { std::mem::zeroed() };
            let emit_status = unsafe {
                emit(
                    audio.context,
                    &NativeAudioEmitRequest {
                        signal_id: NativeUtf8Slice {
                            bytes: signal_id.as_ptr(),
                            len: signal_id.len(),
                        },
                        descriptor: NativeAudioSourceDescriptor {
                            clip,
                            bus: NativeAudioBus::Sfx,
                            volume: 0.5,
                            pitch: 1.0,
                            looping: false,
                            spatial_blend: 0.0,
                            max_distance: 1.0,
                            rolloff: NativeAudioRolloff::Linear,
                            pan: 0.0,
                            emitter_kind: NativeAudioEmitterKind::Global2d,
                            position: NativeVec3::default(),
                            entity: 0,
                            offset: NativeVec3::default(),
                        },
                    },
                    &mut signal,
                    &mut operation_error,
                )
            };
            if emit_status != ABI_OK {
                return emit_status;
            }
            AUDIO_DISPOSE_CLIP.store(clip.value, Ordering::SeqCst);
            AUDIO_DISPOSE_DESTROY_STATUS.store(-1, Ordering::SeqCst);
            AUDIO_DISPOSE_FIXTURE_ACTIVE.store(true, Ordering::SeqCst);
        }
        // SAFETY: the fixture provides a non-null opaque value which is never
        // dereferenced by its callbacks.
        unsafe { *handle = std::ptr::NonNull::<u8>::dangling().as_ptr().cast() };
        ABI_OK
    }

    unsafe extern "C" fn drop_fixture_action(_handle: *mut c_void) -> i32 {
        ABI_OK
    }

    unsafe extern "C" fn drop_fixture_shutdown(_handle: *mut c_void) -> i32 {
        record_drop_event("shutdown");
        DROP_CALLBACK_STATUS.load(Ordering::SeqCst)
    }

    unsafe extern "C" fn drop_fixture_update(
        _handle: *mut c_void,
        _args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        UPDATE_CALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
        if UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC.swap(false, Ordering::SeqCst) {
            let publish = FIXTURE_DIAGNOSTICS_PUBLISH.load(Ordering::SeqCst);
            let context = FIXTURE_DIAGNOSTICS_CONTEXT.load(Ordering::SeqCst);
            let request = NativeDiagnosticsPublishRequest {
                severity: NativeDiagnosticsSeverity::Info,
                disposition: NativeDiagnosticsDisposition::Accepted,
                source: fixture_utf8(b"fixture"),
                code: fixture_utf8(b"FIXTURE_IMMEDIATE_MUTATION"),
                message: fixture_utf8(b"committed before callback escape"),
                correlation: fixture_utf8(b"fixture-update"),
            };
            // SAFETY: `drop_fixture_create` captured this function table from
            // the currently loaded runtime; the call occurs synchronously
            // inside its update callback while the service set remains live.
            let publish: NativePublishDiagnostics = unsafe { std::mem::transmute(publish) };
            let status = unsafe { publish(context as *mut c_void, &request) };
            UPDATE_CALLBACK_DIAGNOSTIC_STATUS.store(status, Ordering::SeqCst);
        }
        if UPDATE_CALLBACK_PUBLISH_UI.swap(false, Ordering::SeqCst) {
            publish_fixture_ui_projection();
        }
        // SAFETY: the fixture owns the provided writable result pointer.
        unsafe { *result = NativeProductUpdateResult::None };
        UPDATE_CALLBACK_STATUS.load(Ordering::SeqCst)
    }

    /// Opens one UI stream and publishes `true` on it, as a product HUD would.
    fn publish_fixture_ui_projection() {
        let context = FIXTURE_UI_CONTEXT.load(Ordering::SeqCst) as *mut c_void;
        // SAFETY: `drop_fixture_create` captured these functions from the
        // loaded runtime; this runs synchronously inside its update callback.
        let open: NativeOpenUiStream =
            unsafe { std::mem::transmute(FIXTURE_UI_OPEN.load(Ordering::SeqCst)) };
        let publish: NativePublishUiProjection =
            unsafe { std::mem::transmute(FIXTURE_UI_PUBLISH.load(Ordering::SeqCst)) };
        let request = NativeUiStreamRequest {
            stream: fixture_utf8(b"fixture.hud"),
            contract: fixture_utf8(b"fixture.hud.v1"),
        };
        let mut stream = NativeUiStreamHandle::default();
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe { open(context, &request, &mut stream, &mut receipt) },
            ABI_OK
        );
        let node = NativeStructuredValueNode {
            kind: NativeStructuredValueKind::Bool,
            bool_value: 1,
            number_value: 0.0,
            key_offset: 0,
            key_len: 0,
            text_offset: 0,
            text_len: 0,
            first_edge: 0,
            child_count: 0,
        };
        let projection = NativeUiProjection {
            stream,
            sequence: 1,
            value: NativeStructuredValue {
                nodes: &node,
                node_count: 1,
                edges: std::ptr::null(),
                edge_count: 0,
                root: 0,
                utf8: std::ptr::null(),
                utf8_len: 0,
            },
        };
        assert_eq!(
            unsafe { publish(context, &projection, &mut receipt) },
            ABI_OK
        );
    }

    unsafe extern "C" fn voxel_failure_fixture_create(
        args: *const NativeProductCreateArgs,
        handle: *mut *mut c_void,
        error: *mut NativeProductCallError,
    ) -> i32 {
        let status = unsafe { drop_fixture_create(args, handle, error) };
        if status == ABI_OK {
            // SAFETY: product creation receives the live Engine service table.
            // The focused fixture uses these function pointers only during one
            // synchronous update callback in this same runtime incarnation.
            let engine = unsafe { (*args).engine };
            VOXEL_FAILURE_SPATIAL_CONTEXT.store(engine.spatial.context as usize, Ordering::SeqCst);
            VOXEL_FAILURE_DESTROY_SESSION
                .store(engine.spatial.destroy_session as usize, Ordering::SeqCst);
            VOXEL_FAILURE_VOXEL_CONTEXT.store(engine.voxel.context as usize, Ordering::SeqCst);
            VOXEL_FAILURE_APPLY_EDITS.store(engine.voxel.apply_edits as usize, Ordering::SeqCst);
            VOXEL_FAILURE_PRESENTATION_CONTEXT.store(
                engine.voxel_scene_presentation.context as usize,
                Ordering::SeqCst,
            );
            VOXEL_FAILURE_REFRESH_SCENE.store(
                engine.voxel_scene_presentation.refresh_scene as usize,
                Ordering::SeqCst,
            );
        }
        status
    }

    unsafe extern "C" fn voxel_failure_fixture_update(
        _handle: *mut c_void,
        _args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        UPDATE_CALLBACK_CALLS.fetch_add(1, Ordering::SeqCst);
        if VOXEL_FAILURE_DESTROY_SESSION_ENABLED.swap(false, Ordering::SeqCst) {
            // SAFETY: the fixture stores the spatial function pointer from its
            // own live Engine table and invokes it synchronously in this
            // generated product callback.
            let destroy: NativeDestroySpatialSession = unsafe {
                std::mem::transmute(VOXEL_FAILURE_DESTROY_SESSION.load(Ordering::SeqCst))
            };
            let status = unsafe {
                destroy(
                    VOXEL_FAILURE_SPATIAL_CONTEXT.load(Ordering::SeqCst) as *mut c_void,
                    NativeSpatialSessionHandle {
                        value: VOXEL_FAILURE_SESSION.load(Ordering::SeqCst),
                    },
                )
            };
            VOXEL_FAILURE_DESTROY_SESSION_STATUS.store(status, Ordering::SeqCst);
        }
        if VOXEL_FAILURE_ENABLED.swap(false, Ordering::SeqCst) {
            // SAFETY: the fixture stores exact function pointers from its own
            // live Engine table in create, and invokes them synchronously in
            // this generated product callback.
            let apply: NativeApplyVoxelEdits =
                unsafe { std::mem::transmute(VOXEL_FAILURE_APPLY_EDITS.load(Ordering::SeqCst)) };
            let refresh: NativeRefreshVoxelScenePresentation =
                unsafe { std::mem::transmute(VOXEL_FAILURE_REFRESH_SCENE.load(Ordering::SeqCst)) };
            let clear = [NativeVoxelEdit {
                state: 0,
                kind: NativeVoxelEditKind::Clear,
                address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
                material_slot: 0,
            }];
            let mut receipt = NativeVoxelEditReceipt::default();
            let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
            let apply_status = unsafe {
                apply(
                    VOXEL_FAILURE_VOXEL_CONTEXT.load(Ordering::SeqCst) as *mut c_void,
                    &NativeVoxelEditTransaction {
                        session: NativeSpatialSessionHandle {
                            value: VOXEL_FAILURE_SESSION.load(Ordering::SeqCst),
                        },
                        edits: clear.as_ptr(),
                        edits_len: clear.len(),
                    },
                    &mut receipt,
                    &mut error,
                )
            };
            VOXEL_FAILURE_APPLY_STATUS.store(apply_status, Ordering::SeqCst);
            let mut readout = NativeVoxelScenePresentationReadout::default();
            let refresh_status = unsafe {
                refresh(
                    VOXEL_FAILURE_PRESENTATION_CONTEXT.load(Ordering::SeqCst) as *mut c_void,
                    NativeVoxelScenePresentationHandle {
                        value: VOXEL_FAILURE_PRESENTATION.load(Ordering::SeqCst),
                    },
                    &mut readout,
                    &mut std::mem::zeroed::<NativeOperationErrorReceipt>(),
                )
            };
            VOXEL_FAILURE_REFRESH_STATUS.store(refresh_status, Ordering::SeqCst);
            if apply_status != ABI_OK || refresh_status != ABI_OK || receipt.accepted_revision != 2
            {
                return 0;
            }
        }
        // SAFETY: the fixture owns the provided writable result pointer.
        unsafe { *result = NativeProductUpdateResult::None };
        UPDATE_CALLBACK_STATUS.load(Ordering::SeqCst)
    }

    unsafe extern "C" fn drop_fixture_timeline(
        _handle: *mut c_void,
        _completion: *const NativeProductTimelineCompletion,
        accepted: *mut u8,
    ) -> i32 {
        // SAFETY: the fixture owns the provided writable acceptance pointer.
        unsafe { *accepted = 0 };
        ABI_OK
    }

    unsafe extern "C" fn drop_fixture_destroy(_handle: *mut c_void) {
        if AUDIO_DISPOSE_FIXTURE_ACTIVE.swap(false, Ordering::SeqCst) {
            let destroy: NativeDestroyAudioClip =
                unsafe { std::mem::transmute(AUDIO_DISPOSE_AUDIO_DESTROY.load(Ordering::SeqCst)) };
            let mut operation_error = unsafe { std::mem::zeroed() };
            let status = unsafe {
                destroy(
                    AUDIO_DISPOSE_AUDIO_CONTEXT.load(Ordering::SeqCst) as *mut c_void,
                    NativeAudioClipHandle {
                        value: AUDIO_DISPOSE_CLIP.load(Ordering::SeqCst),
                    },
                    &mut operation_error,
                )
            };
            AUDIO_DISPOSE_DESTROY_STATUS.store(status, Ordering::SeqCst);
        }
        record_drop_event("destroy");
    }

    unsafe extern "C" fn product_error_fixture_create(
        _args: *const NativeProductCreateArgs,
        handle: *mut *mut c_void,
        result: *mut NativeProductCallError,
    ) -> i32 {
        let service = b"Animation";
        let operation = b"OpenAnimatedMesh";
        let message = b"CSHARP_ANIMATION_RESOURCE_UNKNOWN: missing-diagnostic.glb";
        // SAFETY: the fixture writes the caller-owned output pointers and
        // exposes static bytes until the matching release callback.
        unsafe {
            *handle = std::ptr::NonNull::<u8>::dangling().as_ptr().cast();
            *result = NativeProductCallError {
                service: NativeUtf8Slice {
                    bytes: service.as_ptr(),
                    len: service.len(),
                },
                operation: NativeUtf8Slice {
                    bytes: operation.as_ptr(),
                    len: operation.len(),
                },
                status: 0,
                message: NativeUtf8Slice {
                    bytes: message.as_ptr(),
                    len: message.len(),
                },
            };
        }
        99
    }

    unsafe extern "C" fn product_error_fixture_read(
        _handle: *mut c_void,
        result: *mut NativeProductCallError,
    ) -> i32 {
        // SAFETY: the fixture receives the call helper's writable result.
        unsafe { *result = NativeProductCallError::default() };
        ABI_OK
    }

    unsafe extern "C" fn product_error_fixture_release(
        _handle: *mut c_void,
        _result: NativeProductCallError,
    ) {
        PRODUCT_ERROR_RELEASES.fetch_add(1, Ordering::SeqCst);
    }

    fn product_error_fixture_api() -> LoadedProductApi {
        let mut api = drop_fixture_api();
        api.create = product_error_fixture_create;
        api.read_call_error = product_error_fixture_read;
        api.release_call_error = product_error_fixture_release;
        api
    }

    const FIXTURE_DEBUG_CATALOG: &[u8] = br#"{"available":true,"commands":[]}"#;

    unsafe extern "C" fn fixture_describe_debug(
        _handle: *mut c_void,
        result: *mut NativeProductDebugResult,
    ) -> i32 {
        // SAFETY: the fixture receives the call helper's writable result and
        // exposes a static catalog until its matching release.
        unsafe {
            *result = NativeProductDebugResult {
                succeeded: 1,
                message: NativeUtf8Slice {
                    bytes: FIXTURE_DEBUG_CATALOG.as_ptr(),
                    len: FIXTURE_DEBUG_CATALOG.len(),
                },
            };
        }
        ABI_OK
    }

    unsafe extern "C" fn fixture_release_debug_result(
        _handle: *mut c_void,
        _result: NativeProductDebugResult,
    ) {
    }

    unsafe extern "C" fn fixture_observe_runtime(
        _handle: *mut c_void,
        _facts: *const NativeProductRuntimeFacts,
    ) {
    }

    unsafe extern "C" fn fixture_read_call_error(
        _handle: *mut c_void,
        result: *mut NativeProductCallError,
    ) -> i32 {
        // SAFETY: the fixture receives the call helper's writable result.
        unsafe { *result = NativeProductCallError::default() };
        ABI_OK
    }

    unsafe extern "C" fn fixture_release_call_error(
        _handle: *mut c_void,
        _result: NativeProductCallError,
    ) {
    }

    fn drop_fixture_api() -> LoadedProductApi {
        LoadedProductApi {
            host: LoadedProductHost::NativeAot(None),
            create: drop_fixture_create,
            start: drop_fixture_action,
            update: drop_fixture_update,
            complete_timeline: drop_fixture_timeline,
            paused_intents: paused_intent_fixture,
            pause: drop_fixture_action,
            resume: drop_fixture_action,
            restart: drop_fixture_action,
            shutdown: drop_fixture_shutdown,
            destroy: drop_fixture_destroy,
            execute_debug: debug_semantic_failure,
            describe_debug: fixture_describe_debug,
            release_debug_result: fixture_release_debug_result,
            observe_runtime: fixture_observe_runtime,
            read_call_error: fixture_read_call_error,
            release_call_error: fixture_release_call_error,
        }
    }

    fn direct_input_fixture_api() -> LoadedProductApi {
        let mut api = drop_fixture_api();
        api.update = direct_input_fixture_update;
        api
    }

    fn remapping_callback_fixture_api() -> LoadedProductApi {
        let mut api = direct_input_fixture_api();
        api.create = remapping_callback_fixture_create;
        api.update = remapping_callback_fixture_update;
        api
    }

    fn voxel_failure_fixture_api() -> LoadedProductApi {
        let mut api = drop_fixture_api();
        api.create = voxel_failure_fixture_create;
        api.update = voxel_failure_fixture_update;
        api
    }

    fn drop_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        drop_fixture_runtime_with_config(label, RuntimeLifecycleConfig::Demand)
    }

    fn audio_disposal_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(root.join("audio")).expect("audio fixture content root");
        let mut wav = vec![0_u8; 44];
        wav[..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&36_u32.to_le_bytes());
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16_u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1_u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1_u16.to_le_bytes());
        wav[24..28].copy_from_slice(&4_u32.to_le_bytes());
        wav[28..32].copy_from_slice(&4_u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2_u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16_u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&0_u32.to_le_bytes());
        fs::write(root.join("audio/dispose.wav"), wav).expect("audio fixture WAV");
        let content = CsharpProductContent::admit(&root).expect("audio fixture content");
        AUDIO_DISPOSE_FIXTURE_ENABLED.store(true, Ordering::SeqCst);
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                Vec::new(),
            ),
            || Ok(drop_fixture_api()),
        )
        .expect("audio disposal fixture runtime");
        (runtime, root)
    }

    fn realtime_drop_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        let config = RealtimeLifecycleConfig::new(30, 2).expect("realtime fixture config");
        drop_fixture_runtime_with_config(label, RuntimeLifecycleConfig::Realtime(config))
    }

    fn drop_fixture_runtime_with_config(
        label: &str,
        lifecycle_config: RuntimeLifecycleConfig,
    ) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("drop fixture content root");
        let content = CsharpProductContent::admit(&root).expect("drop fixture content");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                lifecycle_config,
                Vec::new(),
            ),
            || Ok(drop_fixture_api()),
        )
        .expect("drop fixture runtime");
        (runtime, root)
    }

    fn drop_fixture_runtime_with_diagnostics(
        label: &str,
        diagnostics: ProductHostLog,
    ) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("drop fixture content root");
        let content = CsharpProductContent::admit(&root).expect("drop fixture content");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                Vec::new(),
            )
            .with_diagnostics(diagnostics),
            || Ok(drop_fixture_api()),
        )
        .expect("drop fixture runtime");
        (runtime, root)
    }

    fn direct_input_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("direct-input fixture content root");
        let content = CsharpProductContent::admit(&root).expect("direct-input fixture content");
        let descriptor = DirectInputIntentDescriptor::product_payload(
            "fixture.product.payload",
            "fixture.product.payload.v1",
        )
        .expect("direct-input descriptor");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                vec![descriptor],
            ),
            || Ok(direct_input_fixture_api()),
        )
        .expect("direct-input fixture runtime");
        (runtime, root)
    }

    fn remapping_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("remapping fixture content root");
        let content = CsharpProductContent::admit(&root).expect("remapping fixture content");
        let descriptor =
            DirectInputIntentDescriptor::new("fixture.attack", IntentValueKind::Digital)
                .expect("remapping descriptor");
        let old_mapping = RuntimeInputMapping::new(
            "attack-w",
            "fixture.attack",
            RuntimeInputTrigger::Key {
                code: runtime_input_model::KeyboardControl::KeyW,
                edge: InputEdge::Pressed,
                chord: Vec::new(),
                context: None,
            },
        )
        .expect("old remapping fixture mapping");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                vec![descriptor],
            )
            .with_physical_mappings(vec![old_mapping]),
            || Ok(direct_input_fixture_api()),
        )
        .expect("remapping fixture runtime");
        (runtime, root)
    }

    fn callback_remapping_fixture_runtime(label: &str) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("callback remapping fixture content root");
        let content =
            CsharpProductContent::admit(&root).expect("callback remapping fixture content");
        let descriptor =
            DirectInputIntentDescriptor::new("fixture.attack", IntentValueKind::Digital)
                .expect("callback remapping descriptor");
        let old_mapping = RuntimeInputMapping::new(
            "attack-w",
            "fixture.attack",
            RuntimeInputTrigger::Key {
                code: runtime_input_model::KeyboardControl::KeyW,
                edge: InputEdge::Pressed,
                chord: Vec::new(),
                context: None,
            },
        )
        .expect("callback old mapping");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                vec![descriptor],
            )
            .with_physical_mappings(vec![old_mapping]),
            || Ok(remapping_callback_fixture_api()),
        )
        .expect("callback remapping runtime");
        (runtime, root)
    }

    fn voxel_failure_fixture_runtime_with_diagnostics(
        label: &str,
        diagnostics: ProductHostLog,
    ) -> (CsharpProductRuntime, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("voxel failure fixture content root");
        let content = CsharpProductContent::admit(&root).expect("voxel failure fixture content");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                Vec::new(),
            )
            .with_diagnostics(diagnostics),
            || Ok(voxel_failure_fixture_api()),
        )
        .expect("voxel failure fixture runtime");
        (runtime, root)
    }

    fn commit_voxel_presentation_for_recovery(
        runtime: &mut CsharpProductRuntime,
    ) -> (
        NativeSpatialSessionHandle,
        NativeVoxelScenePresentationHandle,
    ) {
        runtime.services.begin_call(ui_binding(&runtime.lifecycle));
        let api = runtime.services.api();
        let mut session = NativeSpatialSessionHandle::default();
        assert_eq!(
            unsafe {
                (api.spatial.create_session)(
                    api.spatial.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let set = [NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 0, y: 0, z: 0 },
            material_slot: 1,
        }];
        let mut voxel_receipt = NativeVoxelEditReceipt::default();
        let mut voxel_error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.voxel.apply_edits)(
                    api.voxel.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: set.as_ptr(),
                        edits_len: set.len(),
                    },
                    &mut voxel_receipt,
                    &mut voxel_error,
                )
            },
            ABI_OK
        );
        let mut material = NativeMaterialHandle::default();
        assert_eq!(
            unsafe {
                (api.graphics.create_material)(
                    api.graphics.context,
                    NativeMaterialRequest {
                        texture_scale: NativeVec2::default(),
                        texture_offset: NativeVec2::default(),
                        stochastic_tiling: 0.0,
                        shader: Default::default(),
                        triplanar_sharpness: 0.0,
                        color: NativeColor {
                            r: 0.25,
                            g: 0.5,
                            b: 0.75,
                            a: 1.0,
                        },
                        texture: NativeRenderResourceReference::default(),
                        roughness: 1.0,
                        metalness: 0.0,
                        normal_map: NativeRenderResourceReference::default(),
                        normal_scale: 1.0,
                        texture_tint: NativeColor {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        emission_color: NativeVec3::default(),
                        emission_intensity: 0.0,
                        double_sided: false,
                        alpha_mode: NativeMaterialAlphaMode::Opaque,
                        alpha_cutoff: 0.5,
                        emission_map: Default::default(),
                        occlusion_map: Default::default(),
                        orm_map: Default::default(),
                        occlusion_strength: 0.0,
                        unlit: false,
                        flat_shading: false,
                        wind_bend: 0.0,
                        wind_flutter: 0.0,
                        water: Default::default(),
                        translucent_shadow: false,
                    },
                    &mut material,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let bindings = [NativeVoxelSceneMaterialBinding {
            material_slot: 1,
            material,
        }];
        let mut presentation = NativeVoxelScenePresentationHandle::default();
        assert_eq!(
            unsafe {
                (api.voxel_scene_presentation.project_scene)(
                    api.voxel_scene_presentation.context,
                    &NativeProjectVoxelSceneRequest {
                        session,
                        materials: bindings.as_ptr(),
                        materials_len: bindings.len(),
                    },
                    &mut presentation,
                    &mut std::mem::zeroed::<NativeOperationErrorReceipt>(),
                )
            },
            ABI_OK
        );
        let mut staged = runtime
            .services
            .finish_call()
            .expect("initial voxel projection");
        assert!(service_outputs(staged.take_output())
            .expect("initial output")
            .iter()
            .any(|output| publication_kind(output) == "frame"));
        (session, presentation)
    }

    #[test]
    fn fresh_graphics_and_voxel_baseline_uses_committed_world_without_product_callbacks() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DROP_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        let (mut runtime, root) = drop_fixture_runtime_with_diagnostics(
            "presentation-world-baseline",
            ProductHostLog::new(Default::default()).unwrap(),
        );
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        let (session, presentation) = commit_voxel_presentation_for_recovery(&mut runtime);
        runtime.services.begin_call(ui_binding(&runtime.lifecycle));
        let api = runtime.services.api();
        let mut appearance = NativeAppearanceHandle::default();
        assert_eq!(
            unsafe {
                (api.graphics.create_primitive)(
                    api.graphics.context,
                    NativePrimitiveAppearanceRequest {
                        geometry: NativePrimitiveGeometry::Cube,
                        wireframe: false,
                        color: NativeColor {
                            r: 1.0,
                            g: 0.5,
                            b: 0.25,
                            a: 1.0,
                        },
                    },
                    &mut appearance,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        let fact = NativeAppearanceFact {
            object_id: 42,
            has_parent_object: false,
            parent_object_id: 0,
            appearance,
            visible: true,
            layer: NativeRenderLayer::Scene,
            transform: NativeTransform {
                translation: NativeVec3 {
                    x: 3.0,
                    y: 0.0,
                    z: 0.0,
                },
                rotation: NativeQuat {
                    w: 1.0,
                    ..Default::default()
                },
                scale: NativeVec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            },
            shadow_casting: Default::default(),
        };
        assert_eq!(
            unsafe {
                (api.graphics.publish_snapshot)(
                    api.graphics.context,
                    &fact,
                    1,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        runtime.services.finish_call().unwrap();
        let callbacks = DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let binding = runtime.binding();
        let frontier = runtime.services.renderer_publication_frontiers();
        let (_, first) = runtime.connect().unwrap().into_parts();
        let (_, second) = runtime.connect().unwrap().into_parts();
        assert_eq!(
            *DROP_EVENTS.lock().unwrap_or_else(PoisonError::into_inner),
            callbacks,
            "attachment invokes no product code"
        );
        assert_eq!(runtime.binding(), binding);
        assert_eq!(runtime.services.renderer_publication_frontiers(), frontier);
        assert_eq!(first, second);
        let baseline = complete_voxel_baseline(&first).unwrap();
        // All required operation kinds spread across separate frames must not
        // accidentally satisfy the single-frame fixture contract.
        let source_frame: render_model::RenderFrameDiff =
            serde_json::from_value(baseline.clone()).unwrap();
        let split: Vec<_> = source_frame
            .ops
            .iter()
            .map(|op| {
                RuntimePublication::Frame(
                    render_model::RenderFrameDiff::try_from_ops(vec![op.clone()]).unwrap(),
                )
            })
            .collect();
        let error = complete_voxel_baseline(&split).unwrap_err().to_string();
        assert!(error.contains("observed [defineMaterial], missing [create, replaceMeshPayload]"));
        assert!(error.contains("observed [create], missing [defineMaterial, replaceMeshPayload]"));
        assert!(error.contains("observed [replaceMeshPayload], missing [defineMaterial, create]"));
        assert!(
            baseline["ops"]
                .as_array()
                .unwrap()
                .iter()
                .any(|op| op["node"]["metadata"]["sourceEntity"] == 42),
            "ordinary graphics joins voxel baseline"
        );

        runtime.services.begin_call(ui_binding(&runtime.lifecycle));
        let api = runtime.services.api();
        let edit = NativeVoxelEdit {
            state: 0,
            kind: NativeVoxelEditKind::Set,
            address: NativeVoxelAddress { x: 1, y: 0, z: 0 },
            material_slot: 1,
        };
        let mut receipt = NativeVoxelEditReceipt::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        assert_eq!(
            unsafe {
                (api.voxel.apply_edits)(
                    api.voxel.context,
                    &NativeVoxelEditTransaction {
                        session,
                        edits: &edit,
                        edits_len: 1,
                    },
                    &mut receipt,
                    &mut error,
                )
            },
            ABI_OK
        );
        let mut readout = NativeVoxelScenePresentationReadout::default();
        assert_eq!(
            unsafe {
                (api.voxel_scene_presentation.refresh_scene)(
                    api.voxel_scene_presentation.context,
                    presentation,
                    &mut readout,
                    &mut std::mem::zeroed::<NativeOperationErrorReceipt>(),
                )
            },
            ABI_OK
        );
        let mut call = runtime.services.finish_call().unwrap();
        let output = call.take_output();
        let delta = output
            .frames
            .iter()
            .find(|frame| !frame.ops.is_empty())
            .unwrap();
        let publication = delta.publication.as_ref().unwrap();
        assert_eq!(publication.stream, "presentation-world");
        assert_eq!(publication.base_revision, frontier[0].1);
        assert!(serde_json::to_value(delta).unwrap()["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "replaceMeshPayload"));
        let (_, latest) = runtime.connect().unwrap().into_parts();
        assert_ne!(complete_voxel_baseline(&latest).unwrap(), baseline);
        assert_eq!(
            *DROP_EVENTS.lock().unwrap_or_else(PoisonError::into_inner),
            callbacks
        );
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn voxel_exercise_diagnostic_distinguishes_absent_and_empty_frames() {
        let absent = complete_voxel_baseline(&[]).unwrap_err().to_string();
        assert!(absent.contains("no frame publications observed"));
        assert!(absent.contains("missing [defineMaterial, create, replaceMeshPayload]"));
        assert!(absent.contains("Products without voxel content should launch without --exercise"));
        let empty = RuntimePublication::Frame(render_model::RenderFrameDiff::new());
        let error = complete_voxel_baseline(&[empty]).unwrap_err().to_string();
        assert!(error.contains(
            "frame 1: observed [], missing [defineMaterial, create, replaceMeshPayload]"
        ));
        assert!(!error.contains("no frame publications observed"));
    }

    #[test]
    fn configured_runtime_incarnation_reaches_readout_and_binding() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let root = content_fixture_root("configured-runtime-incarnation");
        fs::create_dir_all(&root).expect("create fixture content root");
        let content = CsharpProductContent::admit(&root).expect("admit fixture content");
        let runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(77),
                RuntimeLifecycleConfig::Demand,
                Vec::new(),
            ),
            || Ok(drop_fixture_api()),
        )
        .expect("load configured fixture runtime");

        assert_eq!(runtime.readout().runtime().instance_id.get(), 77);
        assert_eq!(runtime.binding().instance_id.get(), 77);

        drop(runtime);
        fs::remove_dir_all(root).expect("remove fixture content");
    }

    #[test]
    fn escaped_update_faults_keeps_the_product_and_resume_continues() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let diagnostics = ProductHostLog::new(Default::default()).expect("fixture diagnostics");
        UPDATE_CALLBACK_CALLS.store(0, Ordering::SeqCst);
        UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC.store(true, Ordering::SeqCst);
        UPDATE_CALLBACK_DIAGNOSTIC_STATUS.store(0, Ordering::SeqCst);
        UPDATE_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC.store(false, Ordering::SeqCst);
        UPDATE_CALLBACK_PUBLISH_UI.store(true, Ordering::SeqCst);
        let (mut runtime, root) =
            drop_fixture_runtime_with_diagnostics("faulted-update", diagnostics.clone());
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("fixture start");
        // An ordinary update publishes the HUD before the throwing one.
        runtime.admit_demand_step().expect("ordinary update");
        assert_eq!(
            runtime
                .services
                .snapshot_ui_projections(ui_binding(&runtime.lifecycle))
                .len(),
            1
        );
        UPDATE_CALLBACK_CALLS.store(0, Ordering::SeqCst);
        UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC.store(true, Ordering::SeqCst);
        UPDATE_CALLBACK_STATUS.store(99, Ordering::SeqCst);
        let (_, outputs) = runtime
            .admit_demand_step()
            .expect("an escaped exception faults instead of failing the operation")
            .into_parts();
        assert_eq!(runtime.lifecycle.state(), RuntimeState::Faulted);
        assert!(
            outputs
                .iter()
                .any(|output| publication_kind(output) == "complete-baseline"),
            "renderers get a fresh baseline after the fault"
        );
        // The browser clears UI when the binding changes, so the fault
        // binding carries the retained HUD projection with it.
        let kinds: Vec<_> = outputs
            .iter()
            .map(|output| publication_kind(output).to_owned())
            .collect();
        let binding = kinds.iter().rposition(|kind| kind == "binding").unwrap();
        assert!(
            kinds[binding..].iter().any(|kind| kind == "ui-projection"),
            "fault outputs after the binding: {kinds:?}"
        );
        assert_eq!(
            UPDATE_CALLBACK_DIAGNOSTIC_STATUS.load(Ordering::SeqCst),
            ABI_OK
        );
        assert!(
            diagnostics
                .snapshot()
                .events
                .iter()
                .any(|event| event.code() == "FIXTURE_IMMEDIATE_MUTATION"),
            "what the call did before the exception is kept"
        );
        runtime
            .admit_demand_step()
            .expect_err("a faulted lifecycle admits no simulation");
        assert_eq!(UPDATE_CALLBACK_CALLS.load(Ordering::SeqCst), 1);

        UPDATE_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        runtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .expect("resume continues the same product");
        runtime.admit_demand_step().expect("simulation continues");
        assert_eq!(UPDATE_CALLBACK_CALLS.load(Ordering::SeqCst), 2);
        drop(runtime);
        let events = DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert!(events.contains(&"shutdown"));
        assert!(events.contains(&"destroy"));
        fs::remove_dir_all(root).expect("remove faulted fixture content");
    }

    fn long_product_error() -> &'static str {
        static MESSAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        MESSAGE.get_or_init(|| {
            let mut message = String::from("System.InvalidOperationException: fixture failure\n");
            for frame in 0..64 {
                message.push_str(&format!(
                    "   at Fixture.Frame{frame}() in /src/Fixture.cs:line {frame}\n"
                ));
            }
            message
        })
    }

    unsafe extern "C" fn long_product_error_read(
        _handle: *mut c_void,
        result: *mut NativeProductCallError,
    ) -> i32 {
        let message = long_product_error().as_bytes();
        // SAFETY: the fixture writes the call helper's result and exposes
        // static bytes until the matching release callback.
        unsafe {
            *result = NativeProductCallError {
                service: NativeUtf8Slice::default(),
                operation: NativeUtf8Slice::default(),
                status: 99,
                message: NativeUtf8Slice {
                    bytes: message.as_ptr(),
                    len: message.len(),
                },
            };
        }
        ABI_OK
    }

    #[test]
    fn a_fault_line_is_the_first_detail_line_bounded() {
        let failure = CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_CALL",
            format!(
                "product callback returned status 99: {}",
                long_product_error()
            ),
        );
        assert_eq!(
            fault_line("update", &failure),
            "rusty: product update faulted: CSHARP_PRODUCT_CALL: product callback returned status 99: System.InvalidOperationException: fixture failure"
        );
        let long = CsharpProductRuntimeError::new("CSHARP_PRODUCT_CALL", "é".repeat(2_000));
        let line = fault_line("start", &long);
        assert!(line.ends_with("..."));
        assert!(!line.contains('\n'));
        assert!(line.chars().count() < 900);
    }

    #[test]
    fn long_multiline_product_error_is_reported_whole_without_panicking() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let diagnostics = ProductHostLog::new(Default::default()).expect("fixture diagnostics");
        UPDATE_CALLBACK_PUBLISH_DIAGNOSTIC.store(false, Ordering::SeqCst);
        UPDATE_CALLBACK_STATUS.store(99, Ordering::SeqCst);
        let (mut runtime, root) =
            drop_fixture_runtime_with_diagnostics("long-product-error", diagnostics.clone());
        runtime.api.read_call_error = long_product_error_read;
        runtime.api.release_call_error = product_error_fixture_release;
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("fixture start");
        runtime
            .admit_demand_step()
            .expect("a failed callback still returns its receipt");
        UPDATE_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        assert_eq!(runtime.lifecycle.state(), RuntimeState::Faulted);
        assert!(long_product_error().len() > 1_024);
        assert!(
            diagnostics
                .snapshot()
                .events
                .iter()
                .any(|event| event.message().contains(long_product_error())),
            "the complete multiline product error reaches runtime diagnostics"
        );
        drop(runtime);
        fs::remove_dir_all(root).expect("remove long-error fixture content");
    }

    #[test]
    fn exception_after_a_voxel_edit_publishes_the_edit_and_a_fresh_baseline() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let diagnostics = ProductHostLog::new(Default::default()).expect("fixture diagnostics");
        UPDATE_CALLBACK_CALLS.store(0, Ordering::SeqCst);
        UPDATE_CALLBACK_STATUS.store(99, Ordering::SeqCst);
        VOXEL_FAILURE_DESTROY_SESSION_ENABLED.store(false, Ordering::SeqCst);
        VOXEL_FAILURE_DESTROY_SESSION_STATUS.store(0, Ordering::SeqCst);
        VOXEL_FAILURE_APPLY_STATUS.store(0, Ordering::SeqCst);
        VOXEL_FAILURE_REFRESH_STATUS.store(0, Ordering::SeqCst);
        let (mut runtime, root) =
            voxel_failure_fixture_runtime_with_diagnostics("voxel-edit-exception", diagnostics);
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start voxel failure fixture");
        let (session, presentation) = commit_voxel_presentation_for_recovery(&mut runtime);
        VOXEL_FAILURE_SESSION.store(session.value, Ordering::SeqCst);
        VOXEL_FAILURE_PRESENTATION.store(presentation.value, Ordering::SeqCst);
        VOXEL_FAILURE_ENABLED.store(true, Ordering::SeqCst);
        let (_, outputs) = runtime
            .admit_demand_step()
            .expect("the fault still publishes the call's work")
            .into_parts();
        VOXEL_FAILURE_ENABLED.store(false, Ordering::SeqCst);
        UPDATE_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        assert_eq!(VOXEL_FAILURE_APPLY_STATUS.load(Ordering::SeqCst), ABI_OK);
        assert_eq!(VOXEL_FAILURE_REFRESH_STATUS.load(Ordering::SeqCst), ABI_OK);
        let encoded = outputs.iter().map(publication_value).collect::<Vec<_>>();
        assert!(encoded.iter().any(|output| {
            output["kind"] == "frame"
                && output["frame"]["ops"]
                    .as_array()
                    .is_some_and(|ops| ops.iter().any(|operation| operation["op"] == "destroy"))
        }));
        assert!(encoded
            .iter()
            .any(|output| output["kind"] == "complete-baseline"));
        assert_eq!(runtime.lifecycle.state(), RuntimeState::Faulted);
        drop(runtime);
        fs::remove_dir_all(root).expect("remove voxel edit fixture content");
    }

    unsafe extern "C" fn debug_semantic_failure(
        _handle: *mut c_void,
        _command: *const NativeUtf8Slice,
        result: *mut NativeProductDebugResult,
    ) -> i32 {
        let message = b"unknown command";
        // SAFETY: fixture receives the call helper's writable out pointer and
        // exposes a static byte string until its matching fixture release.
        unsafe {
            *result = NativeProductDebugResult {
                succeeded: 0,
                message: NativeUtf8Slice {
                    bytes: message.as_ptr(),
                    len: message.len(),
                },
            };
        }
        ABI_OK
    }

    unsafe extern "C" fn debug_success(
        _handle: *mut c_void,
        _command: *const NativeUtf8Slice,
        result: *mut NativeProductDebugResult,
    ) -> i32 {
        let message = b"fixture command executed";
        // SAFETY: fixture receives the call helper's writable out pointer and
        // exposes a static byte string until its matching fixture release.
        unsafe {
            *result = NativeProductDebugResult {
                succeeded: 1,
                message: NativeUtf8Slice {
                    bytes: message.as_ptr(),
                    len: message.len(),
                },
            };
        }
        ABI_OK
    }

    unsafe extern "C" fn debug_abi_failure_after_result(
        _handle: *mut c_void,
        _command: *const NativeUtf8Slice,
        result: *mut NativeProductDebugResult,
    ) -> i32 {
        let message = b"allocated before failure";
        // SAFETY: fixture receives the call helper's writable out pointer and
        // deliberately initializes it before an ABI failure.
        unsafe {
            *result = NativeProductDebugResult {
                succeeded: 1,
                message: NativeUtf8Slice {
                    bytes: message.as_ptr(),
                    len: message.len(),
                },
            };
        }
        99
    }

    unsafe extern "C" fn release_debug_fixture(
        _handle: *mut c_void,
        _result: NativeProductDebugResult,
    ) {
        DEBUG_RELEASES.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn debug_callback_preserves_semantic_failure_and_releases_once_after_abi_failure() {
        let _guard = DEBUG_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DEBUG_RELEASES.store(0, Ordering::SeqCst);

        let success = call_debug(
            debug_success,
            release_debug_fixture,
            ptr::null_mut(),
            "fixture.count",
        )
        .expect("successful debug result");
        assert!(success.succeeded());
        assert_eq!(success.message(), "fixture command executed");
        assert_eq!(DEBUG_RELEASES.load(Ordering::SeqCst), 1);

        let semantic = call_debug(
            debug_semantic_failure,
            release_debug_fixture,
            ptr::null_mut(),
            "fixture.unknown",
        )
        .expect("semantic debug result");
        assert!(!semantic.succeeded());
        assert_eq!(semantic.message(), "unknown command");
        assert_eq!(DEBUG_RELEASES.load(Ordering::SeqCst), 2);

        let error = call_debug(
            debug_abi_failure_after_result,
            release_debug_fixture,
            ptr::null_mut(),
            "fixture.unknown",
        )
        .expect_err("ABI failure remains a runtime error");
        assert_eq!(error.code(), "CSHARP_PRODUCT_CALL");
        assert_eq!(DEBUG_RELEASES.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn debug_callback_failure_is_reported_and_the_product_continues() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _debug_guard = DEBUG_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("debug-fault-latch");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime.api.execute_debug = debug_semantic_failure;
        runtime.api.release_debug_result = release_debug_fixture;
        assert!(!runtime
            .execute_debug("unknown")
            .unwrap()
            .result()
            .succeeded());
        runtime
            .admit_demand_step()
            .expect("semantic rejection preserves the owner");
        runtime.api.execute_debug = debug_abi_failure_after_result;
        assert_eq!(
            runtime
                .execute_debug("mutating-command")
                .unwrap_err()
                .code(),
            "CSHARP_PRODUCT_CALL"
        );
        runtime
            .admit_demand_step()
            .expect("a failed debug command does not stop the product");
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    unsafe extern "C" fn timeline_error_fixture(
        _handle: *mut c_void,
        _completion: *const NativeProductTimelineCompletion,
        _accepted: *mut u8,
    ) -> i32 {
        99
    }

    unsafe extern "C" fn timeline_error_read(
        _handle: *mut c_void,
        result: *mut NativeProductCallError,
    ) -> i32 {
        let mut ignored = ptr::null_mut();
        // SAFETY: reuse the fixture's static named diagnostic, with local outputs.
        unsafe {
            product_error_fixture_create(ptr::null(), &mut ignored, result);
        }
        ABI_OK
    }

    #[test]
    fn timeline_failure_preserves_named_call_error_and_false_is_not_failure() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut api = drop_fixture_api();
        let completion = NativeProductTimelineCompletion {
            ticket: 1,
            instance_id: 1,
            generation: 1,
            control_revision: 1,
            correlation: NativeUtf8Slice::default(),
            outcome: NativeProductTimelineOutcome::Success,
            outcome_data: NativeByteSlice {
                bytes: ptr::null(),
                len: 0,
            },
            provenance_correlation: NativeUtf8Slice::default(),
            provenance_detail: NativeByteSlice {
                bytes: ptr::null(),
                len: 0,
            },
        };
        assert!(!call_complete_timeline(&api, ptr::null_mut(), &completion).unwrap());
        api.complete_timeline = timeline_error_fixture;
        api.read_call_error = timeline_error_read;
        api.release_call_error = product_error_fixture_release;
        let error = call_complete_timeline(&api, ptr::null_mut(), &completion).unwrap_err();
        assert!(error
            .detail()
            .contains("Animation.OpenAnimatedMesh returned status 0"));
        assert!(error.detail().contains("CSHARP_ANIMATION_RESOURCE_UNKNOWN"));
    }

    #[test]
    fn failed_create_copies_named_engine_diagnostic_and_destroys_the_product() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        PRODUCT_ERROR_RELEASES.store(0, Ordering::SeqCst);
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        let root = content_fixture_root("product-create-diagnostic");
        fs::create_dir_all(&root).expect("product error fixture content root");
        let content = CsharpProductContent::admit(&root).expect("product error fixture content");
        let error = match CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Demand,
                Vec::new(),
            ),
            || Ok(product_error_fixture_api()),
        ) {
            Ok(_) => panic!("named Engine create failure must reject the product"),
            Err(error) => error,
        };

        assert_eq!(error.code(), "CSHARP_PRODUCT_CALL");
        assert!(error
            .detail()
            .contains("Animation.OpenAnimatedMesh returned status 0"));
        assert!(error
            .detail()
            .contains("CSHARP_ANIMATION_RESOURCE_UNKNOWN: missing-diagnostic.glb"));
        assert_eq!(PRODUCT_ERROR_RELEASES.load(Ordering::SeqCst), 1);
        assert_eq!(
            DROP_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            ["destroy"]
        );
        fs::remove_dir_all(root).expect("remove product error fixture content");
    }

    #[test]
    fn implicit_shutdown_runs_before_product_disposal() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DROP_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (runtime, root) = drop_fixture_runtime("implicit-shutdown-success");
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        drop(runtime);
        assert_eq!(
            DROP_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            ["shutdown", "destroy"],
        );
        fs::remove_dir_all(root).expect("remove drop fixture content");
    }

    #[test]
    fn shutdown_releases_silenced_one_shot_before_product_disposal() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DROP_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        AUDIO_DISPOSE_FIXTURE_ENABLED.store(false, Ordering::SeqCst);
        AUDIO_DISPOSE_FIXTURE_ACTIVE.store(false, Ordering::SeqCst);
        AUDIO_DISPOSE_DESTROY_STATUS.store(-1, Ordering::SeqCst);
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (runtime, root) = audio_disposal_fixture_runtime("audio-disposal-shutdown");

        drop(runtime);
        assert_eq!(AUDIO_DISPOSE_DESTROY_STATUS.load(Ordering::SeqCst), ABI_OK);
        assert_eq!(
            DROP_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            ["shutdown", "destroy"],
        );
        fs::remove_dir_all(root).expect("remove audio disposal fixture content");
    }

    #[test]
    fn failed_implicit_shutdown_still_disposes_the_product() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DROP_CALLBACK_STATUS.store(41, Ordering::SeqCst);
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (runtime, root) = drop_fixture_runtime("implicit-shutdown-failure");
        DROP_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        drop(runtime);
        assert_eq!(
            DROP_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            ["shutdown", "destroy"],
        );
        DROP_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        fs::remove_dir_all(root).expect("remove drop fixture content");
    }

    #[test]
    fn direct_product_payload_is_snapshot_delivered_once_after_preceding_clear() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (mut runtime, root) = direct_input_fixture_runtime("direct-payload-snapshot");

        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start direct-input fixture");
        runtime
            .admit_demand_step()
            .expect("drain start clear before the regression sequence");
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        let binding = input_binding(&runtime.lifecycle);
        let payload = payload_intent(
            binding,
            2,
            "fixture.product.payload",
            "fixture.product.payload.v1",
        )
        .expect("fixture payload intent");
        runtime
            .input(ProductHostInputBatch::new(vec![
                input_clear(binding, 1),
                payload,
            ]))
            .expect("admit ordered clear and direct payload");
        assert!(
            DIRECT_INPUT_CALLBACK_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_empty(),
            "input admission must not call the product before an admitted snapshot"
        );

        runtime
            .admit_demand_step()
            .expect("deliver ordered clear and direct payload");
        assert_eq!(
            DIRECT_INPUT_CALLBACK_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_slice(),
            &[vec![
                DirectInputCallbackEvent {
                    kind: NativeInputEventKind::Clear,
                    provenance: NativeInputProvenance::Physical,
                    phase: NativeInputPhase::None,
                    sequence: 1,
                    intent: Vec::new(),
                    payload_contract: Vec::new(),
                    payload_data: Vec::new(),
                },
                DirectInputCallbackEvent {
                    kind: NativeInputEventKind::DirectProductPayload,
                    provenance: NativeInputProvenance::DirectUi,
                    phase: NativeInputPhase::DirectUi,
                    sequence: 2,
                    intent: b"fixture.product.payload".to_vec(),
                    payload_contract: b"fixture.product.payload.v1".to_vec(),
                    payload_data: br#"{"exercise":true}"#.to_vec(),
                },
            ]],
            "the admitted generated callback receives the clear then one direct semantic payload"
        );

        drop(runtime);
        fs::remove_dir_all(root).expect("remove direct-input fixture content");
    }

    #[test]
    fn lifecycle_baseline_reconstructs_effects_without_replaying_their_delta_revision() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = direct_input_fixture_runtime("effect-baseline-frontier");
        runtime.services.begin_call(ui_binding(&runtime.lifecycle));
        let api = runtime.services.api();
        assert_eq!(
            unsafe {
                (api.audio.set_bus_muted)(
                    api.audio.context,
                    &NativeAudioBusMutedRequest {
                        bus: NativeAudioBus::Sfx,
                        muted: true,
                    },
                    std::ptr::null_mut(),
                )
            },
            1
        );
        let mut call = runtime.services.finish_call().unwrap();
        let deltas = service_outputs(call.take_output()).unwrap();
        let baseline = runtime.tag_complete_baseline(deltas).unwrap();
        let encoded = baseline.iter().map(publication_value).collect::<Vec<_>>();
        assert!(encoded
            .iter()
            .filter(|output| output["kind"] == "presentation")
            .all(|output| output["frame"].get("publication").is_none()));
        assert!(encoded.iter().any(|output| {
            output["kind"] == "presentation"
                && output["frame"]["ops"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|op| op["op"]["control"]["muted"] == true)
        }));
        assert!(encoded.iter().any(|output| output["kind"] == "frame"));
        assert_eq!(runtime.services.renderer_publication_frontiers()[0].1, 1);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_input_returns_recoverable_cursor_without_replaying_the_event() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (mut runtime, root) = direct_input_fixture_runtime("duplicate-input-recovery");

        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start duplicate-input fixture");
        runtime
            .admit_demand_step()
            .expect("drain start clear before duplicate input");
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        let binding = input_binding(&runtime.lifecycle);
        let first = input_clear(binding, 1);
        runtime
            .input(ProductHostInputBatch::new(vec![first.clone()]))
            .expect("admit first input");
        let warnings = runtime.diagnostics.snapshot().warning_count;
        let duplicate = runtime
            .input(ProductHostInputBatch::new(vec![first]))
            .expect("safe duplicate returns a typed receipt");
        assert_eq!(
            runtime.diagnostics.snapshot().warning_count,
            warnings,
            "a safe drop is not a warning"
        );
        let result = duplicate.result();
        assert!(!result.is_accepted());
        let encoded = serde_json::to_value(result).expect("duplicate receipt serializes");
        assert_eq!(encoded["code"], "CSHARP_INPUT_STALE_DROPPED");
        assert_eq!(encoded["disposition"], "rejected-recoverable");
        assert_eq!(encoded["acceptedCount"], 0);
        assert_eq!(encoded["droppedCount"], 1);
        assert_eq!(encoded["acceptedThrough"], serde_json::Value::Null);
        assert_eq!(encoded["consumedThrough"], "1");
        assert_eq!(encoded["nextInputSequence"], "2");

        runtime
            .admit_demand_step()
            .expect("deliver only the first input");
        let events = DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].len(), 1);
        assert_eq!(events[0][0].kind, NativeInputEventKind::Clear);
        assert_eq!(events[0][0].sequence, 1);

        drop(runtime);
        fs::remove_dir_all(root).expect("remove duplicate-input fixture content");
    }

    #[test]
    fn settled_mapping_replacement_fences_old_browser_input_and_delivers_fresh_edges() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (mut runtime, root) = remapping_fixture_runtime("mapping-replacement-fence");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start fixture");
        runtime.admit_demand_step().expect("drain start clear");
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();

        let old_binding = input_binding(&runtime.lifecycle);
        let old_press = RuntimeInputEvent::Physical(RuntimeInputIngress::new(
            old_binding,
            1,
            standard_input_context(),
            RuntimeInputFact::Key {
                code: runtime_input_model::KeyboardControl::KeyW,
                edge: runtime_input::PhysicalEdge::Pressed,
            },
        ));
        runtime
            .input(ProductHostInputBatch::new(vec![old_press]))
            .expect("queue old physical edge");
        let replacement = CompiledInputMappings::standard(
            runtime.direct_intents.clone(),
            [RuntimeInputMapping::new(
                "attack-f",
                "fixture.attack",
                RuntimeInputTrigger::Key {
                    code: runtime_input_model::KeyboardControl::KeyF,
                    edge: InputEdge::Pressed,
                    chord: Vec::new(),
                    context: None,
                },
            )
            .expect("new mapping")],
        )
        .expect("compile replacement");
        runtime
            .settle_input_mapping_replacement(replacement)
            .expect("settle replacement");
        let fresh_binding = input_binding(&runtime.lifecycle);
        assert_ne!(fresh_binding, old_binding);
        assert_eq!(
            runtime.pending_inputs.len(),
            1,
            "old queued facts are replaced by one clear"
        );
        let stale = runtime
            .input(ProductHostInputBatch::new(vec![
                RuntimeInputEvent::Physical(RuntimeInputIngress::new(
                    old_binding,
                    2,
                    standard_input_context(),
                    RuntimeInputFact::Key {
                        code: runtime_input_model::KeyboardControl::KeyW,
                        edge: runtime_input::PhysicalEdge::Pressed,
                    },
                )),
            ]))
            .expect("old binding is a safe stale drop");
        assert!(
            !stale.result().is_accepted(),
            "old-binding inflight input cannot be remapped"
        );
        runtime
            .input(ProductHostInputBatch::new(vec![
                RuntimeInputEvent::Physical(RuntimeInputIngress::new(
                    fresh_binding,
                    1,
                    standard_input_context(),
                    RuntimeInputFact::Key {
                        code: runtime_input_model::KeyboardControl::KeyF,
                        edge: runtime_input::PhysicalEdge::Pressed,
                    },
                )),
            ]))
            .expect("fresh physical edge is admitted");
        runtime.admit_demand_step().expect("deliver fresh edge");
        let events = DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .iter()
            .any(|event| event.kind == NativeInputEventKind::MappedDigital
                && event.intent == b"fixture.attack"));
        assert!(!events[0].iter().any(|event| event.intent == b"attack-w"));
        drop(runtime);
        fs::remove_dir_all(root).expect("remove remapping fixture content");
    }

    #[test]
    fn update_callback_staged_mapping_rebinds_the_browser_and_delivers_fresh_edges() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        REMAPPING_CALLBACK_STAGE.store(false, Ordering::SeqCst);
        REMAPPING_CALLBACK_STATUS.store(0, Ordering::SeqCst);
        REMAPPING_CALLBACK_OUTCOME.store(0, Ordering::SeqCst);
        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let (mut runtime, root) =
            callback_remapping_fixture_runtime("callback-mapping-replacement");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start fixture");
        runtime.admit_demand_step().expect("drain start clear");
        let old_binding = input_binding(&runtime.lifecycle);
        REMAPPING_CALLBACK_STAGE.store(true, Ordering::SeqCst);
        let (_, outputs) = runtime
            .admit_demand_step()
            .expect("settle callback mapping")
            .into_parts();
        assert_eq!(REMAPPING_CALLBACK_STATUS.load(Ordering::SeqCst), ABI_OK);
        assert_eq!(
            REMAPPING_CALLBACK_OUTCOME.load(Ordering::SeqCst) as u32,
            NativeInputMappingReplacementOutcome::Staged as u32
        );
        let fresh_binding = input_binding(&runtime.lifecycle);
        assert_ne!(fresh_binding, old_binding);
        assert!(outputs.iter().any(|output| matches!(output, RuntimePublication::Binding { runtime, .. } if *runtime == fresh_binding)));
        assert!(outputs.iter().any(|output| matches!(output, RuntimePublication::CompleteBaseline { runtime, .. } if *runtime == fresh_binding)));
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, RuntimePublication::Frame(_))),
            "a mapping replacement rebinds in place without a world snapshot"
        );
        assert_eq!(
            runtime.pending_inputs.len(),
            1,
            "callback replacement leaves one clear fact"
        );

        DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        runtime
            .input(ProductHostInputBatch::new(vec![
                RuntimeInputEvent::Physical(RuntimeInputIngress::new(
                    fresh_binding,
                    1,
                    standard_input_context(),
                    RuntimeInputFact::Key {
                        code: runtime_input_model::KeyboardControl::KeyF,
                        edge: runtime_input::PhysicalEdge::Pressed,
                    },
                )),
            ]))
            .expect("fresh binding input");
        runtime
            .admit_demand_step()
            .expect("deliver fresh mapped edge");
        let events = DIRECT_INPUT_CALLBACK_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert_eq!(events.len(), 1);
        assert!(events[0]
            .iter()
            .any(|event| event.kind == NativeInputEventKind::MappedDigital
                && event.intent == b"fixture.attack"));
        drop(runtime);
        fs::remove_dir_all(root).expect("remove callback remapping fixture content");
    }

    #[test]
    fn debug_result_rejects_invalid_utf8_and_callback_pair_requires_both_members() {
        let invalid_utf8 = NativeProductDebugResult {
            succeeded: 1,
            message: NativeUtf8Slice {
                bytes: b"\xff".as_ptr(),
                len: 1,
            },
        };
        assert_eq!(
            copy_debug_result(invalid_utf8)
                .expect_err("invalid UTF-8 result")
                .code(),
            "CSHARP_DEBUG_RESULT_UTF8"
        );
        // Past the former 64 KiB result bound.
        let large = vec![b'x'; 100 * 1024];
        let copied = copy_debug_result(NativeProductDebugResult {
            succeeded: 1,
            message: NativeUtf8Slice {
                bytes: large.as_ptr(),
                len: large.len(),
            },
        })
        .expect("large result");
        assert_eq!(copied.message().len(), large.len());
    }

    const HANDSHAKE_FIXTURE_IDENTITY: &[u8] = b"fixture-sdk/v1";

    unsafe extern "C" fn matching_native_bind(
        host: *const NativeProductAbiHandshakeV1,
        product: *mut NativeProductAbiHandshakeV1,
    ) -> i32 {
        let mut response = unsafe { *host };
        response.build_identity = NativeUtf8Slice {
            bytes: HANDSHAKE_FIXTURE_IDENTITY.as_ptr(),
            len: HANDSHAKE_FIXTURE_IDENTITY.len(),
        };
        response.product_api = Box::into_raw(Box::new(NativeProductApi::default()));
        unsafe { *product = response };
        ABI_OK
    }

    unsafe extern "system" fn matching_coreclr_bind(
        host: *const NativeProductAbiHandshakeV1,
        product: *mut NativeProductAbiHandshakeV1,
    ) -> i32 {
        unsafe { matching_native_bind(host, product) }
    }

    unsafe extern "C" fn version_mismatch_bind(
        host: *const NativeProductAbiHandshakeV1,
        product: *mut NativeProductAbiHandshakeV1,
    ) -> i32 {
        unsafe { matching_native_bind(host, product) };
        unsafe { (*product).protocol_version += 1 };
        ABI_OK
    }

    unsafe extern "C" fn table_size_mismatch_bind(
        host: *const NativeProductAbiHandshakeV1,
        product: *mut NativeProductAbiHandshakeV1,
    ) -> i32 {
        unsafe { matching_native_bind(host, product) };
        unsafe { (*product).product_api_size += 8 };
        ABI_OK
    }

    unsafe extern "C" fn fingerprint_mismatch_bind(
        host: *const NativeProductAbiHandshakeV1,
        product: *mut NativeProductAbiHandshakeV1,
    ) -> i32 {
        unsafe { matching_native_bind(host, product) };
        unsafe { (*product).fingerprint.word0 ^= 1 };
        ABI_OK
    }

    fn assert_handshake_rejection(bind: NativeProductBindV1) {
        let error =
            product_from_bind(bind).expect_err("mismatched ABI must reject before table copy");
        assert_eq!(error.code(), "CSHARP_PRODUCT_ABI_MISMATCH");
        assert!(error.detail().contains("expected protocol="));
        assert!(error.detail().contains("observed protocol="));
        assert!(error
            .detail()
            .contains("restore the matching runtime pack or rebuild the product"));
    }

    #[test]
    fn v1_handshake_accepts_matching_nativeaot_and_coreclr_binds() {
        product_from_bind(matching_native_bind).expect("matching NativeAOT V1 bind");
        product_from_coreclr_bind(matching_coreclr_bind).expect("matching CoreCLR V1 bind");
    }

    #[test]
    fn v1_handshake_rejects_version_table_size_and_fingerprint_without_writing_a_sentinel() {
        let sentinel = NativeProductApi {
            create: Some(drop_fixture_create),
            ..NativeProductApi::default()
        };
        let before = sentinel.create.expect("sentinel callback") as usize;
        assert_handshake_rejection(version_mismatch_bind);
        assert_handshake_rejection(table_size_mismatch_bind);
        assert_handshake_rejection(fingerprint_mismatch_bind);
        assert_eq!(
            sentinel.create.expect("sentinel callback") as usize,
            before,
            "a mismatched product never receives host-owned product-table storage"
        );
    }

    #[test]
    fn service_frame_storage_moves_into_its_publication() {
        let frame =
            render_model::RenderFrameDiff::try_from_ops(vec![render_model::RenderDiff::Create {
                handle: render_model::RenderHandle::new(1),
                parent: None,
                node: render_model::RenderNode::new(render_model::Geometry::Cube),
            }])
            .unwrap();
        let operations = frame.ops.as_ptr();
        let output = CsharpEngineCallOutput {
            frames: vec![frame],
            ..Default::default()
        };
        let publication = service_outputs(output).unwrap().pop().unwrap();
        let RuntimePublication::Frame(frame) = publication else {
            panic!("frame publication");
        };
        assert_eq!(
            frame.ops.as_ptr(),
            operations,
            "owned operations must not be deep-copied in transit"
        );
    }

    fn content_fixture_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "csharp-product-runtime-{label}-{}-{}",
            std::process::id(),
            CONTENT_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn service_outputs_preserve_appearance_frame_and_presentation_order() {
        let output = CsharpEngineCallOutput {
            render_output: Vec::new(),
            appearance: vec![
                CsharpAppearanceCallOutput::Presentation(
                    render_presentation::PresentationFrameDiff::new(),
                ),
                CsharpAppearanceCallOutput::Frame(Default::default()),
            ],
            frames: Vec::new(),
            view_composition: None,
            ui: Vec::new(),
            presentation: Vec::new(),
        };
        let encoded = service_outputs(output)
            .expect("ordered service output")
            .into_iter()
            .map(|output| publication_kind(&output).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(encoded, ["presentation", "frame"]);
    }

    #[test]
    fn content_collection_leaves_unselected_png_bytes_unvalidated() {
        let root = content_fixture_root("content");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("content root");
        fs::write(root.join("unrelated-ui.png"), b"not an RGBA PNG").expect("content file");

        let content = CsharpProductContent::admit(&root)
            .expect("collect content without admitting resources");
        fs::remove_dir_all(&root).expect("remove fixture");

        assert_eq!(content.files.len(), 1);
    }

    #[test]
    fn content_admission_excludes_independent_bundle_payloads() {
        let root = content_fixture_root("bundle-content");
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(root.join("legacy.txt"), b"legacy").unwrap();
        fs::write(root.join("rules/body.bin"), b"not the inventory hash").unwrap();
        fs::write(root.join(".rusty-bundles.json"), format!(
            r#"{{"bundles":[{{"id":"rules","root":"rules","files":[{{"path":"body.bin","byteLength":8,"sha256":"{}"}}]}}]}}"#,
            "0".repeat(64))).unwrap();
        let content = CsharpProductContent::admit(&root).unwrap();
        assert_eq!(content.files.len(), 1);
        assert_eq!(content.files[0].path, b"legacy.txt");
        assert!(content.bundles.owns_path("rules/body.bin"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn content_admission_sorts_canonical_nested_paths() {
        let root = content_fixture_root("sorted-content");
        fs::create_dir_all(root.join("nested")).expect("nested content root");
        fs::write(root.join("z-last.txt"), b"z").expect("last content file");
        fs::write(root.join("nested").join("middle.txt"), b"middle").expect("nested content file");
        fs::write(root.join("a-first.txt"), b"a").expect("first content file");

        let content = CsharpProductContent::admit(&root).expect("admit valid content");
        fs::remove_dir_all(&root).expect("remove fixture");

        assert_eq!(
            content
                .files
                .iter()
                .map(|file| String::from_utf8(file.path.clone()).expect("UTF-8 path"))
                .collect::<Vec<_>>(),
            ["a-first.txt", "nested/middle.txt", "z-last.txt"]
        );
    }

    #[test]
    fn content_admission_accepts_file_above_old_mesh_derived_limit() {
        let root = content_fixture_root("large-content");
        fs::create_dir_all(&root).unwrap();
        let size = 64 * 1024 * 1024 + 1;
        fs::File::create(root.join("large.bin"))
            .unwrap()
            .set_len(size)
            .unwrap();
        let content = CsharpProductContent::admit(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(content.files.len(), 1);
        assert_eq!(content.files[0].bytes.len() as u64, size);
    }

    #[test]
    fn content_admission_rejects_legacy_normalization_collision() {
        let root = content_fixture_root("noncanonical-content");
        fs::create_dir_all(root.join("nested")).expect("content root");
        fs::write(root.join("nested").join("file.txt"), b"canonical path")
            .expect("canonical content path");
        fs::write(root.join("nested\\file.txt"), b"ambiguous path")
            .expect("noncanonical content path");

        let error = CsharpProductContent::admit(&root)
            .err()
            .expect("reject backslash component");
        fs::remove_dir_all(&root).expect("remove fixture");

        assert_eq!(error.code(), "CSHARP_CONTENT_PATH");
    }

    #[cfg(unix)]
    #[test]
    fn content_admission_rejects_symlinks_and_special_entries() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let root = content_fixture_root("symlink-content");
        let outside = content_fixture_root("outside-content");
        fs::create_dir_all(&root).expect("content root");
        fs::write(&outside, b"outside content").expect("outside content file");
        symlink(&outside, root.join("linked-file")).expect("symlink fixture");

        let error = CsharpProductContent::admit(&root)
            .err()
            .expect("reject file symlink");
        assert_eq!(error.code(), "CSHARP_CONTENT_ENTRY");
        fs::remove_file(&outside).expect("remove outside content");
        fs::remove_dir_all(&root).expect("remove symlink fixture");

        let root = content_fixture_root("special-content");
        fs::create_dir_all(&root).expect("content root");
        let socket = UnixListener::bind(root.join("content.socket")).expect("socket fixture");

        let error = CsharpProductContent::admit(&root)
            .err()
            .expect("reject socket entry");
        assert_eq!(error.code(), "CSHARP_CONTENT_ENTRY");
        drop(socket);
        fs::remove_dir_all(&root).expect("remove special fixture");
    }

    #[cfg(unix)]
    #[test]
    fn content_admission_rejects_non_utf8_paths_and_symlink_roots() {
        use std::{
            ffi::OsString,
            os::unix::{ffi::OsStringExt, fs::symlink},
        };

        let root = content_fixture_root("nonutf8-content");
        fs::create_dir_all(&root).expect("content root");
        fs::write(
            root.join(OsString::from_vec(b"not-utf8-\xff.txt".to_vec())),
            b"invalid name",
        )
        .expect("non-UTF-8 content file");

        let error = CsharpProductContent::admit(&root)
            .err()
            .expect("reject non-UTF-8 path");
        assert_eq!(error.code(), "CSHARP_CONTENT_PATH");
        fs::remove_dir_all(&root).expect("remove non-UTF-8 fixture");

        let target = content_fixture_root("symlink-root-target");
        let root = content_fixture_root("symlink-root");
        fs::create_dir_all(&target).expect("target content root");
        symlink(&target, &root).expect("root symlink fixture");

        let error = CsharpProductContent::admit(&root)
            .err()
            .expect("reject symlink root");
        assert_eq!(error.code(), "CSHARP_CONTENT_ROOT");
        fs::remove_file(&root).expect("remove root symlink");
        fs::remove_dir_all(&target).expect("remove target root");
    }

    #[test]
    fn lifecycle_readout_reports_each_explicit_standard_mode() {
        let cases = [
            (
                CsharpProductRuntime::standard_realtime_config(),
                product_host::ProductHostRuntimeMode::Realtime,
            ),
            (
                RuntimeLifecycleConfig::Demand,
                product_host::ProductHostRuntimeMode::Demand,
            ),
            (
                RuntimeLifecycleConfig::External,
                product_host::ProductHostRuntimeMode::External,
            ),
        ];
        for (config, expected_mode) in cases {
            let lifecycle = RuntimeLifecycle::new(RuntimeInstanceId::new(1), config);
            assert_eq!(dev_readout(lifecycle.readout()).mode(), expected_mode);
        }
    }

    static MANUAL_UPDATE_FACTS: Mutex<Vec<NativeProductUpdateFacts>> = Mutex::new(Vec::new());

    unsafe extern "C" fn manual_time_fixture_update(
        _handle: *mut c_void,
        args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        // SAFETY: the runtime supplies live update arguments and a writable result for this callback.
        unsafe {
            MANUAL_UPDATE_FACTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((*args).facts);
            *result = NativeProductUpdateResult::None;
        }
        ABI_OK
    }

    #[test]
    fn inspection_advance_delivers_realtime_fixed_steps_to_the_product() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = realtime_drop_fixture_runtime("manual-realtime-facts");
        runtime.api.update = manual_time_fixture_update;
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        for mode in ["manual", "action-driven"] {
            MANUAL_UPDATE_FACTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
            runtime
                .execute_time_debug(&format!("engine.time.mode {mode}"))
                .unwrap();
            runtime
                .execute_time_debug("engine.time.advance 100")
                .unwrap();
            let facts = MANUAL_UPDATE_FACTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            assert_eq!(facts.len(), 3);
            for fact in facts.iter() {
                assert_eq!(fact.mode, NativeProductUpdateMode::Realtime);
                assert_eq!(fact.lifecycle_state, NativeProductLifecycleState::Running);
                assert_eq!(fact.fixed_step_hz, 30);
                assert_eq!(fact.fixed_delta_seconds, 1.0 / 30.0);
                assert_eq!(fact.admitted_step_count, 1);
            }
            assert!(facts
                .windows(2)
                .all(|pair| pair[1].simulation_step > pair[0].simulation_step));
        }
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    /// What the gameplay-time fixture product asks for in its next update.
    #[derive(Clone, Copy)]
    enum FixtureTimeRequest {
        Rate(f64),
        Advance(f64, f64),
    }

    static GAMEPLAY_FIXTURE_REQUEST: Mutex<Option<FixtureTimeRequest>> = Mutex::new(None);
    static GAMEPLAY_FIXTURE_STATUS: Mutex<Vec<(i32, NativeGameplayTimeReadout)>> =
        Mutex::new(Vec::new());

    /// Records its facts like the manual-time fixture, then makes the one
    /// scripted gameplay time request through the generated service table,
    /// as product policy would from live state.
    unsafe extern "C" fn gameplay_time_fixture_update(
        handle: *mut c_void,
        args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        unsafe { manual_time_fixture_update(handle, args, result) };
        let request = GAMEPLAY_FIXTURE_REQUEST
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(request) = request else {
            return ABI_OK;
        };
        let api = FIXTURE_GAMEPLAY_TIME
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .expect("fixture gameplay time table")
            .0;
        let mut readout = NativeGameplayTimeReadout::default();
        let mut error = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };
        // SAFETY: the table and its context are live for this product call.
        let status = unsafe {
            match request {
                FixtureTimeRequest::Rate(rate) => (api.select_rate)(
                    api.context,
                    &NativeGameplayTimeRateRequest { rate },
                    &mut readout,
                    &mut error,
                ),
                FixtureTimeRequest::Advance(seconds, rate) => (api.advance)(
                    api.context,
                    &NativeGameplayTimeAdvanceRequest { seconds, rate },
                    &mut readout,
                    &mut error,
                ),
            }
        };
        GAMEPLAY_FIXTURE_STATUS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((status, readout));
        ABI_OK
    }

    /// The host observation at which a 30 Hz realtime run owes `steps` steps.
    fn at_30_hz(steps: u64) -> u64 {
        (steps * 1_000_000_000).div_ceil(30)
    }

    struct GameplayTimeRun {
        runtime: CsharpProductRuntime,
        observed: u64,
    }

    impl GameplayTimeRun {
        fn request(&self, request: FixtureTimeRequest) {
            *GAMEPLAY_FIXTURE_REQUEST
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(request);
        }

        /// Observes host time `steps` 30 Hz steps later and returns the
        /// updates the product received.
        fn observe(&mut self, steps: u64) -> Vec<NativeProductUpdateFacts> {
            MANUAL_UPDATE_FACTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clear();
            self.observed += steps;
            self.runtime
                .advance_realtime(CanonicalU64::new(at_30_hz(self.observed)))
                .unwrap();
            MANUAL_UPDATE_FACTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }

        fn admitted(&self) -> u64 {
            self.runtime.lifecycle.readout().admitted_simulation_steps()
        }
    }

    fn gameplay_time_run(label: &str) -> (GameplayTimeRun, PathBuf) {
        let (mut runtime, root) = realtime_drop_fixture_runtime(label);
        runtime.api.update = gameplay_time_fixture_update;
        GAMEPLAY_FIXTURE_STATUS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime.advance_realtime(CanonicalU64::new(0)).unwrap();
        (
            GameplayTimeRun {
                runtime,
                observed: 0,
            },
            root,
        )
    }

    #[test]
    fn product_gameplay_time_holds_slows_and_advances_without_wall_time_debt() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut run, root) = gameplay_time_run("gameplay-time");

        // A product that has not selected gameplay time updates only with steps.
        let first = run.observe(1);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].admitted_step_count, 1);
        assert!(!first[0].gameplay_time_selected);
        assert_eq!(first[0].gameplay_rate, 1.0);

        // The product holds the world from an ordinary update. The request
        // applies from the next observation.
        run.request(FixtureTimeRequest::Rate(0.0));
        assert_eq!(run.observe(1)[0].admitted_step_count, 1);
        let held_at = run.admitted();
        // Ten seconds held: an update per observation, no step and no debt.
        for _ in 0..300 {
            let updates = run.observe(1);
            assert_eq!(updates.len(), 1, "one update per observation while held");
            assert_eq!(updates[0].admitted_step_count, 0);
            assert_eq!(updates[0].simulation_step, held_at);
            assert!(updates[0].gameplay_time_selected);
            assert_eq!(updates[0].gameplay_rate, 0.0);
        }
        assert_eq!(run.admitted(), held_at);
        assert!(
            run.runtime.frame_simulation().held,
            "frames report the hold"
        );

        // A tenth of realtime: three steps a second, still an update per
        // observation.
        run.request(FixtureTimeRequest::Rate(0.1));
        run.observe(1);
        assert!(!run.runtime.frame_simulation().held, "slow time moves");
        let slow_from = run.admitted();
        let mut updates = 0;
        for _ in 0..30 {
            updates += run.observe(1).len();
        }
        assert_eq!(updates, 30);
        assert_eq!(run.admitted() - slow_from, 3);

        // A bounded half-second advance: exactly fifteen steps, then held.
        run.request(FixtureTimeRequest::Advance(0.5, 1.0));
        run.observe(1);
        let advance_from = run.admitted();
        let mut last = None;
        for _ in 0..20 {
            last = run.observe(1).last().copied();
        }
        assert_eq!(run.admitted() - advance_from, 15);
        let last = last.unwrap();
        assert_eq!(last.gameplay_rate, 0.0);
        assert_eq!(last.gameplay_advance_remaining_steps, 0);
        let (status, receipt) = GAMEPLAY_FIXTURE_STATUS.lock().unwrap()[2];
        assert_eq!(status, ABI_OK);
        assert_eq!(receipt.advance_remaining_steps, 15);
        assert_eq!(receipt.fixed_step_hz, 30);

        // An invalid request is refused to the product and changes nothing.
        run.request(FixtureTimeRequest::Rate(2.0));
        run.observe(1);
        assert_eq!(GAMEPLAY_FIXTURE_STATUS.lock().unwrap()[3].0, 0);
        assert!(run.runtime.lifecycle.gameplay_time().held());

        // Back to realtime; a request inside a multi-step update leaves that
        // update's steps alone and holds from the next observation.
        // (Three steps of host time admit the fixture's catch-up cap of two.)
        run.request(FixtureTimeRequest::Rate(1.0));
        run.observe(1);
        run.request(FixtureTimeRequest::Rate(0.0));
        let batch = run.observe(3);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].admitted_step_count, 2);
        assert_eq!(run.observe(5)[0].admitted_step_count, 0);

        drop(run);
        fs::remove_dir_all(root).unwrap();
    }

    /// One event as the gameplay-input fixture product saw it.
    #[derive(Debug, Clone, PartialEq)]
    struct SeenEvent {
        kind: NativeInputEventKind,
        intent: Vec<u8>,
        x: f32,
    }

    static GAMEPLAY_INPUT_EVENTS: Mutex<Vec<Vec<SeenEvent>>> = Mutex::new(Vec::new());

    unsafe extern "C" fn gameplay_input_fixture_update(
        handle: *mut c_void,
        args: *const NativeProductUpdateArgs,
        result: *mut NativeProductUpdateResult,
    ) -> i32 {
        // SAFETY: the runtime supplies live update arguments for this call.
        let args = unsafe { &*args };
        let events = unsafe { std::slice::from_raw_parts(args.events, args.event_count) };
        GAMEPLAY_INPUT_EVENTS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(
                events
                    .iter()
                    .map(|event| SeenEvent {
                        kind: event.kind,
                        intent: unsafe { copy_callback_bytes(event.intent, event.intent_len) },
                        x: event.x,
                    })
                    .collect(),
            );
        unsafe { gameplay_time_fixture_update(handle, args, result) }
    }

    fn gameplay_input_run(label: &str) -> (GameplayTimeRun, PathBuf) {
        let root = content_fixture_root(label);
        fs::create_dir_all(&root).expect("gameplay input fixture content root");
        let content = CsharpProductContent::admit(&root).expect("gameplay input fixture content");
        let attack = DirectInputIntentDescriptor::new("fixture.attack", IntentValueKind::Digital)
            .expect("attack descriptor");
        let look = DirectInputIntentDescriptor::new("fixture.look", IntentValueKind::Axis)
            .expect("look descriptor");
        let mappings = vec![
            RuntimeInputMapping::new(
                "attack-e",
                "fixture.attack",
                RuntimeInputTrigger::Key {
                    code: runtime_input_model::KeyboardControl::KeyE,
                    edge: InputEdge::Pressed,
                    chord: Vec::new(),
                    context: None,
                },
            )
            .expect("attack mapping"),
            RuntimeInputMapping::new(
                "look-stick",
                "fixture.look",
                RuntimeInputTrigger::ControllerAxis {
                    axis: runtime_input_model::ControllerAxis::Axis2,
                    context: None,
                },
            )
            .expect("look mapping"),
        ];
        let mut runtime = CsharpProductRuntime::load_admitted_with(
            content,
            CsharpProductRuntimeConfig::new(
                RuntimeInstanceId::new(1),
                RuntimeLifecycleConfig::Realtime(
                    RealtimeLifecycleConfig::new(30, 2).expect("realtime fixture config"),
                ),
                vec![attack, look],
            )
            .with_physical_mappings(mappings),
            || Ok(drop_fixture_api()),
        )
        .expect("gameplay input fixture runtime");
        runtime.api.update = gameplay_input_fixture_update;
        GAMEPLAY_FIXTURE_STATUS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime.advance_realtime(CanonicalU64::new(0)).unwrap();
        (
            GameplayTimeRun {
                runtime,
                observed: 0,
            },
            root,
        )
    }

    fn physical(
        binding: RuntimeInputBinding,
        sequence: u64,
        fact: RuntimeInputFact,
    ) -> RuntimeInputEvent {
        RuntimeInputEvent::Physical(RuntimeInputIngress::new(
            binding,
            sequence,
            standard_input_context(),
            fact,
        ))
    }

    #[test]
    fn unlocked_pointer_facts_reach_csharp_with_their_position() {
        let binding = input_binding(&RuntimeLifecycle::new(
            RuntimeInstanceId::new(1),
            RuntimeLifecycleConfig::Demand,
        ));
        let axis = |value| AxisValue::new(value).unwrap();
        let at = runtime_input::PointerPosition {
            x: axis(0.25),
            y: axis(0.75),
        };
        let click = native_event(&physical(
            binding,
            1,
            RuntimeInputFact::PointerButton {
                button: runtime_input_model::PointerButton::Primary,
                edge: runtime_input::PhysicalEdge::Pressed,
                position: Some(at),
            },
        ))
        .as_native();
        assert_eq!(click.kind, NativeInputEventKind::PointerButton);
        assert!(click.has_position);
        assert_eq!((click.x, click.y), (0.25, 0.75));
        let moved =
            native_event(&physical(binding, 2, RuntimeInputFact::PointerPosition(at))).as_native();
        assert_eq!(moved.kind, NativeInputEventKind::PointerPosition);
        assert_eq!(moved.channel, NativeInputChannel::PointerPosition);
        assert!(moved.has_position);
        let locked = native_event(&physical(
            binding,
            3,
            RuntimeInputFact::PointerButton {
                button: runtime_input_model::PointerButton::Primary,
                edge: runtime_input::PhysicalEdge::Released,
                position: None,
            },
        ))
        .as_native();
        assert!(!locked.has_position);
        assert_eq!((locked.x, locked.y), (0.0, 0.0));
    }

    fn take_seen() -> Vec<Vec<SeenEvent>> {
        std::mem::take(
            &mut *GAMEPLAY_INPUT_EVENTS
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    fn mapped(updates: &[Vec<SeenEvent>], intent: &[u8]) -> Vec<f32> {
        updates
            .iter()
            .flatten()
            .filter(|event| event.intent == intent)
            .map(|event| event.x)
            .collect()
    }

    #[test]
    fn held_gameplay_time_delivers_look_and_controls_once_without_steps() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut run, root) = gameplay_input_run("gameplay-input");
        run.request(FixtureTimeRequest::Rate(0.0));
        run.observe(1);
        let held_at = run.admitted();
        take_seen();

        // Aim and attack while held: a mouse turn, a stick pushed and held,
        // and one press.
        let binding = input_binding(&run.runtime.lifecycle);
        let axis = |value| AxisValue::new(value).unwrap();
        run.runtime
            .input(ProductHostInputBatch::new(vec![
                physical(
                    binding,
                    1,
                    RuntimeInputFact::PointerDelta {
                        x: axis(12.0),
                        y: axis(-3.0),
                    },
                ),
                physical(
                    binding,
                    2,
                    RuntimeInputFact::ControllerAxis {
                        axis: runtime_input_model::ControllerAxis::Axis2,
                        value: axis(0.8),
                    },
                ),
                physical(
                    binding,
                    3,
                    RuntimeInputFact::Key {
                        code: runtime_input_model::KeyboardControl::KeyE,
                        edge: runtime_input::PhysicalEdge::Pressed,
                    },
                ),
            ]))
            .unwrap();
        let facts = run.observe(1);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].admitted_step_count, 0);
        assert!((facts[0].host_elapsed_seconds - 1.0 / 30.0).abs() < 1e-6);
        let first = take_seen();
        assert_eq!(first.len(), 1);
        let kinds = first[0].iter().map(|event| event.kind).collect::<Vec<_>>();
        for kind in [
            NativeInputEventKind::PointerDelta,
            NativeInputEventKind::ControllerAxis,
            NativeInputEventKind::Key,
        ] {
            assert_eq!(
                kinds.iter().filter(|seen| **seen == kind).count(),
                1,
                "{kind:?}"
            );
        }
        assert_eq!(mapped(&first, b"fixture.attack").len(), 1);
        assert!(mapped(&first, b"fixture.look").contains(&0.8));

        // Held a second of host time with no new events: the stick keeps
        // reporting every update, the press and the mouse turn do not repeat,
        // and the world stays at its step.
        for _ in 0..30 {
            let facts = run.observe(1);
            assert_eq!(facts[0].admitted_step_count, 0);
        }
        let held = take_seen();
        assert_eq!(held.len(), 30);
        assert!(held
            .iter()
            .all(|update| mapped(std::slice::from_ref(update), b"fixture.look") == [0.8]));
        assert!(mapped(&held, b"fixture.attack").is_empty());
        assert!(held
            .iter()
            .flatten()
            .all(|event| event.kind != NativeInputEventKind::PointerDelta));
        assert_eq!(run.admitted(), held_at);

        // A physical control resumes (product policy picks the rate); the
        // attack consumed while held is not replayed by the steps.
        run.request(FixtureTimeRequest::Rate(1.0));
        run.observe(1);
        take_seen();
        for _ in 0..3 {
            run.observe(1);
        }
        let resumed = take_seen();
        assert!(run.admitted() > held_at);
        assert!(mapped(&resumed, b"fixture.attack").is_empty());

        // Focus loss clears what was held: the stick stops reporting.
        let binding = input_binding(&run.runtime.lifecycle);
        run.runtime
            .input(ProductHostInputBatch::new(vec![physical(
                binding,
                4,
                RuntimeInputFact::Clear {
                    reason: InputClearReason::FocusLoss,
                },
            )]))
            .unwrap();
        run.request(FixtureTimeRequest::Rate(0.0));
        run.observe(1);
        run.observe(1);
        assert!(mapped(&take_seen(), b"fixture.look")
            .iter()
            .all(|value| *value == 0.0));

        drop(run);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn gameplay_time_composes_with_pause_inspection_and_restart() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut run, root) = gameplay_time_run("gameplay-time-lifecycle");
        run.request(FixtureTimeRequest::Rate(0.5));
        run.observe(1);
        assert!(run.runtime.lifecycle.gameplay_time().selected());

        // A lifecycle pause stops every update and keeps the product's rate;
        // resuming owes none of the paused wall time.
        run.runtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .unwrap();
        assert_eq!(
            run.runtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Paused
        );
        run.observed += 600;
        run.runtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .unwrap();
        let resumed_at = run.admitted();
        run.observed += 6000;
        run.runtime
            .advance_realtime(CanonicalU64::new(at_30_hz(run.observed)))
            .unwrap();
        assert_eq!(run.admitted(), resumed_at, "a fresh baseline after resume");
        for _ in 0..4 {
            run.observe(1);
        }
        assert_eq!(run.admitted() - resumed_at, 2, "half of four steps");

        // Inspection holds everything and keeps the product's choice.
        run.runtime
            .execute_time_debug("engine.time.mode manual")
            .unwrap();
        assert_eq!(
            run.runtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Held
        );
        assert!(run.observe(30).is_empty());
        let inspected_at = run.admitted();
        run.runtime
            .execute_time_debug("engine.time.advance 100")
            .unwrap();
        assert_eq!(run.admitted() - inspected_at, 3);
        run.runtime
            .execute_time_debug("engine.time.mode realtime")
            .unwrap();
        assert_eq!(
            run.runtime.lifecycle.gameplay_time().rate(),
            runtime_lifecycle::GameplayRate::from_parts_per_million(500_000).unwrap()
        );

        // A restart returns the new generation to realtime, step-only updates.
        run.request(FixtureTimeRequest::Rate(0.0));
        run.observe(1);
        run.runtime
            .lifecycle(ProductHostLifecycleOperation::Restart)
            .unwrap();
        assert!(!run.runtime.lifecycle.gameplay_time().selected());
        run.runtime
            .advance_realtime(CanonicalU64::new(at_30_hz(run.observed)))
            .unwrap();
        let after = run.observe(1);
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].admitted_step_count, 1);
        assert!(!after[0].gameplay_time_selected);

        drop(run);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn realtime_schedule_state_tracks_lifecycle_and_uses_admitted_hz() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut realtime, root) = realtime_drop_fixture_runtime("realtime-schedule-seam");
        assert_eq!(
            realtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Created
        );
        assert_eq!(
            realtime.realtime_schedule_interval(),
            Some(std::time::Duration::from_nanos(33_333_334))
        );

        realtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start realtime fixture");
        assert_eq!(
            realtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Running
        );
        // Pause and resume rebind in place: a new binding and completion, but
        // no world snapshot for the browser to replace its renderer with.
        let in_place = |outputs: &[RuntimePublication]| {
            matches!(outputs.first(), Some(RuntimePublication::Binding { .. }))
                && matches!(
                    outputs.last(),
                    Some(RuntimePublication::CompleteBaseline { .. })
                )
                && !outputs
                    .iter()
                    .any(|output| matches!(output, RuntimePublication::Frame(_)))
        };
        let (_, paused) = realtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .expect("pause realtime fixture")
            .into_parts();
        assert!(in_place(&paused), "pause rebinds in place");
        assert_eq!(
            realtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Paused
        );
        let (_, resumed) = realtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .expect("resume realtime fixture")
            .into_parts();
        assert!(in_place(&resumed), "resume rebinds in place");
        assert_eq!(
            realtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Running
        );
        realtime
            .lifecycle(ProductHostLifecycleOperation::Shutdown)
            .expect("shutdown realtime fixture");
        assert_eq!(
            realtime.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Shutdown
        );
        drop(realtime);
        fs::remove_dir_all(root).expect("remove realtime schedule fixture content");

        let (demand, root) = drop_fixture_runtime("demand-schedule-seam");
        assert_eq!(
            demand.realtime_schedule_state(),
            ProductHostRuntimeScheduleState::Unsupported
        );
        assert_eq!(demand.realtime_schedule_interval(), None);
        drop(demand);
        fs::remove_dir_all(root).expect("remove demand schedule fixture content");
    }

    #[test]
    fn stale_control_and_lifecycle_leave_queued_input_until_an_admitted_fence() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("mailbox-fence-admission");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        let old_binding = runtime.binding();
        runtime
            .control(
                product_host::ProductHostControlOperation::Replace,
                old_binding,
            )
            .unwrap();
        let binding = runtime.binding();
        let owner = product_host::ProductHostOperationOwner::new(runtime);
        let queued = std::cell::Cell::new(2);
        let stale_control = owner.control_with_input_fence(
            product_host::ProductHostControlOperation::Replace,
            old_binding,
            || queued.set(0),
        );
        assert!(stale_control.is_err());
        assert_eq!(queued.get(), 2);
        let stale_lifecycle = owner.lifecycle_with_input_fence(
            ProductHostLifecycleOperation::Pause,
            Some(old_binding),
            || queued.set(0),
        );
        assert!(stale_lifecycle.is_err());
        assert_eq!(queued.get(), 2);
        owner
            .control_with_input_fence(
                product_host::ProductHostControlOperation::Replace,
                binding,
                || queued.set(0),
            )
            .unwrap();
        assert_eq!(queued.get(), 0);
        drop(owner);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn full_wire_batch_reserves_clear_and_following_pressure_returns_scoped_receipt() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("input-pressure-receipt");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        let binding = runtime.binding();
        let batch = |first: usize, count: usize| {
            let events = (first..first + count).map(|sequence| serde_json::json!({
                "runtime": binding, "sequence": sequence.to_string(), "context": "gameplay.default",
                "fact": {"kind": "key", "code": "digit-1", "edge": if sequence % 2 == 1 { "pressed" } else { "released" }},
            })).collect::<Vec<_>>();
            ProductHostInputBatch::decode_json(&serde_json::to_vec(&events).unwrap()).unwrap()
        };
        let count = runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS;
        assert!(runtime
            .input(batch(1, count))
            .unwrap()
            .result()
            .is_accepted());
        assert_eq!(runtime.binding(), binding);
        assert_eq!(runtime.pending_inputs.len(), count + 1);
        let receipt = runtime
            .input(batch(count + 1, 1))
            .expect("safe pressure receipt");
        let result = serde_json::to_value(receipt.result()).unwrap();
        assert_eq!(result["disposition"], "resync-required");
        assert_eq!(result["code"], "CSHARP_INPUT_PENDING_BOUNDS");
        assert_eq!(result["nextInputSequence"], "1");
        assert_eq!(runtime.binding().instance_id, binding.instance_id);
        assert_ne!(runtime.binding().control_revision, binding.control_revision);
        let (_, outputs) = receipt.into_parts();
        assert_eq!(
            publication_kind(outputs.last().unwrap()),
            "complete-baseline"
        );
        runtime.admit_demand_step().expect("owner remains usable");
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_harness_claim_holds_input_until_release_or_its_lease_lapses() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = realtime_drop_fixture_runtime("input-claim");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        let page = runtime.binding();
        let claim_of = |outputs: &[RuntimePublication]| match outputs.first() {
            Some(RuntimePublication::Binding { input_claim, .. }) => input_claim.clone(),
            other => panic!("a control change publishes a binding first, not {other:?}"),
        };

        let refused = |result: Result<_, ProductHostRuntimeError>| match result {
            Err(error) => error.code().to_owned(),
            Ok(_) => panic!("refused"),
        };
        let lease = std::time::Duration::from_secs(60);
        assert_eq!(
            refused(runtime.claim_control(page, " ".to_owned(), lease)),
            "CSHARP_CONTROL_CLAIM"
        );
        assert_eq!(
            refused(runtime.claim_control(page, "agent".to_owned(), std::time::Duration::ZERO)),
            "CSHARP_CONTROL_CLAIM"
        );

        // A claim moves the binding, clears held input, and names its holder.
        let (_, outputs) = runtime
            .claim_control(page, "crew-agent-2".to_owned(), lease)
            .expect("claim")
            .into_parts();
        assert_ne!(runtime.binding(), page, "a claim moves the binding");
        assert_eq!(claim_of(&outputs).as_deref(), Some("crew-agent-2"));
        assert_eq!(runtime.pending_inputs.len(), 1);
        assert_eq!(
            runtime.pending_inputs[0].clear_reason,
            NativeInputClearReason::ControlRevisionChange
        );
        // The page's binding is stale now; a page attaching sees the claim.
        assert!(runtime
            .claim_control(page, "other".to_owned(), lease)
            .is_err());
        let (_, attached) = runtime.connect().unwrap().into_parts();
        assert!(attached.iter().any(|output| matches!(
            output,
            RuntimePublication::Binding { input_claim: Some(label), .. } if label == "crew-agent-2"
        )));

        // Release hands input back to the page.
        let (_, released) = runtime
            .control(ProductHostControlOperation::Release, runtime.binding())
            .unwrap()
            .into_parts();
        assert_eq!(claim_of(&released), None);

        // A lease that lapses without input is released on the next tick.
        runtime
            .claim_control(
                runtime.binding(),
                "crew-agent-3".to_owned(),
                std::time::Duration::from_millis(1),
            )
            .unwrap();
        let held = runtime.binding();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let (_, expired) = runtime
            .advance_realtime(CanonicalU64::new(1_000_000))
            .unwrap()
            .into_parts();
        assert_eq!(claim_of(&expired), None);
        assert_ne!(runtime.binding(), held, "the release moves the binding");
        assert_eq!(
            runtime.pending_inputs.last().unwrap().clear_reason,
            NativeInputClearReason::ControlRevisionChange
        );
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn physical_input_while_paused_is_admitted_and_never_reaches_the_product() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("paused-input");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .unwrap();
        let binding = runtime.binding();
        let events = [serde_json::json!({
            "runtime": binding, "sequence": "1", "context": "gameplay.default",
            "fact": {"kind": "key", "code": "digit-1", "edge": "pressed"},
        })];
        let batch =
            ProductHostInputBatch::decode_json(&serde_json::to_vec(&events).unwrap()).unwrap();
        let pending_before = runtime.pending_inputs.len();
        let receipt = runtime.input(batch).expect("paused input is admitted");
        assert!(receipt.result().is_accepted());
        assert_eq!(
            runtime.pending_inputs.len(),
            pending_before,
            "nothing queued"
        );
        // Resume rebinds: the product's next update sees only the clear.
        runtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .unwrap();
        assert_eq!(runtime.pending_inputs.len(), 1);
        assert_eq!(
            runtime.pending_inputs[0].clear_reason,
            NativeInputClearReason::ControlRevisionChange
        );
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    fn payload_callback_event(sequence: u64) -> DirectInputCallbackEvent {
        DirectInputCallbackEvent {
            kind: NativeInputEventKind::DirectProductPayload,
            provenance: NativeInputProvenance::DirectUi,
            phase: NativeInputPhase::DirectUi,
            sequence,
            intent: b"fixture.product.payload".to_vec(),
            payload_contract: b"fixture.product.payload.v1".to_vec(),
            payload_data: br#"{"exercise":true}"#.to_vec(),
        }
    }

    #[test]
    fn paused_direct_claims_reach_the_product_once_and_never_replay() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = direct_input_fixture_runtime("paused-direct-claims");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime.admit_demand_step().unwrap();
        let running_binding = input_binding(&runtime.lifecycle);
        runtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .unwrap();
        DIRECT_INPUT_CALLBACK_EVENTS.lock().unwrap().clear();
        PAUSED_INTENT_CALLBACK_EVENTS.lock().unwrap().clear();
        let paused_readout = runtime.lifecycle.readout();
        let pending_before = runtime.pending_inputs.len();

        // One batch: a held key (gameplay input) and two claims around it.
        let binding = input_binding(&runtime.lifecycle);
        let key = RuntimeInputEvent::Physical(RuntimeInputIngress::new(
            binding,
            2,
            standard_input_context(),
            RuntimeInputFact::Key {
                code: runtime_input_model::KeyboardControl::KeyW,
                edge: runtime_input::PhysicalEdge::Pressed,
            },
        ));
        let receipt = runtime
            .input(ProductHostInputBatch::new(vec![
                payload_intent(
                    binding,
                    1,
                    "fixture.product.payload",
                    "fixture.product.payload.v1",
                )
                .unwrap(),
                key,
                payload_intent(
                    binding,
                    3,
                    "fixture.product.payload",
                    "fixture.product.payload.v1",
                )
                .unwrap(),
            ]))
            .expect("paused claims are admitted");
        assert!(receipt.result().is_accepted());
        assert_eq!(
            PAUSED_INTENT_CALLBACK_EVENTS.lock().unwrap().as_slice(),
            &[vec![payload_callback_event(1), payload_callback_event(3)]],
            "one call with the claims in order and no physical fact"
        );
        assert!(DIRECT_INPUT_CALLBACK_EVENTS.lock().unwrap().is_empty());
        assert_eq!(
            runtime.pending_inputs.len(),
            pending_before,
            "nothing queued for Update"
        );
        let readout = runtime.lifecycle.readout();
        assert_eq!(readout.state(), RuntimeState::Paused);
        assert_eq!(
            readout.admitted_simulation_steps(),
            paused_readout.admitted_simulation_steps()
        );
        assert_eq!(
            readout.control_revision(),
            paused_readout.control_revision()
        );

        // A claim from before the pause is stale: dropped, never delivered.
        let stale = runtime
            .input(ProductHostInputBatch::new(vec![payload_intent(
                running_binding,
                9,
                "fixture.product.payload",
                "fixture.product.payload.v1",
            )
            .unwrap()]))
            .expect("a stale claim is answered");
        assert_eq!(
            serde_json::to_value(stale.result()).unwrap()["droppedCount"],
            1
        );
        assert_eq!(PAUSED_INTENT_CALLBACK_EVENTS.lock().unwrap().len(), 1);

        // Resume: Update sees only the clear, not the claims or the key.
        runtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .unwrap();
        runtime.admit_demand_step().unwrap();
        let updates = DIRECT_INPUT_CALLBACK_EVENTS.lock().unwrap().clone();
        assert_eq!(updates.len(), 1);
        assert!(updates[0]
            .iter()
            .all(|event| event.kind == NativeInputEventKind::Clear));
        assert_eq!(PAUSED_INTENT_CALLBACK_EVENTS.lock().unwrap().len(), 1);
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_paused_claim_faults_the_runtime_with_a_fresh_input_binding() {
        let _guard = DIRECT_INPUT_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let _drop_guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = direct_input_fixture_runtime("paused-claim-failure");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .unwrap();
        let binding = input_binding(&runtime.lifecycle);
        PAUSED_INTENT_CALLBACK_STATUS.store(99, Ordering::SeqCst);
        let receipt = runtime.input(ProductHostInputBatch::new(vec![payload_intent(
            binding,
            1,
            "fixture.product.payload",
            "fixture.product.payload.v1",
        )
        .unwrap()]));
        PAUSED_INTENT_CALLBACK_STATUS.store(ABI_OK, Ordering::SeqCst);
        let receipt = receipt.expect("the claim was admitted before the product failed");
        assert_eq!(runtime.lifecycle.state(), RuntimeState::Faulted);
        let rebound = input_binding(&runtime.lifecycle);
        assert_ne!(rebound, binding);
        // The cursor is the rebound lane's, not the faulted binding's.
        let result = serde_json::to_value(receipt.result()).unwrap();
        assert_eq!(result["nextInputSequence"], "1");
        assert_eq!(
            result["binding"]["controlRevision"],
            serde_json::to_value(runtime.binding().control_revision).unwrap()
        );
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn paused_mapped_input_is_consumed_without_accumulating() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = remapping_fixture_runtime("paused-mapped-input");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .unwrap();
        runtime
            .lifecycle(ProductHostLifecycleOperation::Pause)
            .unwrap();
        let binding = input_binding(&runtime.lifecycle);
        // W maps to an intent; more presses than the pending-intent capacity.
        for sequence in 1..=2050_u64 {
            let event = RuntimeInputEvent::Physical(RuntimeInputIngress::new(
                binding,
                sequence,
                standard_input_context(),
                RuntimeInputFact::Key {
                    code: runtime_input_model::KeyboardControl::KeyW,
                    edge: if sequence % 2 == 1 {
                        runtime_input::PhysicalEdge::Pressed
                    } else {
                        runtime_input::PhysicalEdge::Released
                    },
                },
            ));
            let receipt = runtime
                .input(ProductHostInputBatch::new(vec![event]))
                .unwrap_or_else(|error| panic!("paused input {sequence} is consumed: {error:?}"));
            assert!(receipt.result().is_accepted(), "paused input {sequence}");
        }
        // Resume: the product's next update sees only the clear.
        runtime
            .lifecycle(ProductHostLifecycleOperation::Resume)
            .unwrap();
        assert_eq!(runtime.pending_inputs.len(), 1);
        assert_eq!(
            runtime.pending_inputs[0].clear_reason,
            NativeInputClearReason::ControlRevisionChange
        );
        drop(runtime);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_input_overflow_rebinds_in_place_without_a_world_snapshot() {
        let _guard = DROP_FIXTURE_GATE
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (mut runtime, root) = drop_fixture_runtime("pending-input-recovery");
        runtime
            .lifecycle(ProductHostLifecycleOperation::Start)
            .expect("start pending-input fixture");
        let previous_binding = runtime.binding();
        let pending_binding = input_binding(&runtime.lifecycle);
        runtime.pending_inputs = (0..MAX_PENDING_NATIVE_INPUTS)
            .map(|_| clear_input_owned(pending_binding, InputClearReason::FocusLoss))
            .collect();

        let admission = runtime
            .append_pending_inputs(vec![clear_input_owned(
                pending_binding,
                InputClearReason::FocusLoss,
            )])
            .expect("overflow is a completed scoped recovery");
        assert_eq!(admission, PendingInputAdmission::Resynchronized);
        assert_ne!(runtime.binding(), previous_binding);
        assert_eq!(runtime.pending_inputs.len(), 1);
        assert_eq!(
            runtime.pending_inputs[0].clear_reason,
            NativeInputClearReason::ControlRevisionChange
        );
        let recovery = runtime
            .pending_recovery_outputs
            .iter()
            .map(publication_value)
            .collect::<Vec<_>>();
        assert_eq!(recovery.first().unwrap()["kind"], "binding");
        assert_eq!(recovery.last().unwrap()["kind"], "complete-baseline");
        // A same-incarnation input fence rebinds in place: no world snapshot.
        assert!(!recovery.iter().any(|output| output["kind"] == "frame"));

        let (_, outputs) = runtime
            .admit_demand_step()
            .expect("next operation remains usable after input recovery")
            .into_parts();
        let encoded = outputs.iter().map(publication_value).collect::<Vec<_>>();
        assert!(encoded.iter().any(|output| output["kind"] == "binding"));
        assert!(
            encoded
                .iter()
                .any(|output| output["kind"] == "complete-baseline"),
            "the next scheduled receipt must carry the retained recovery baseline"
        );
        assert!(runtime.pending_recovery_outputs.is_empty());

        drop(runtime);
        fs::remove_dir_all(root).expect("remove pending input recovery fixture content");
    }

    #[test]
    fn lifecycle_readout_projects_owner_fault_and_restart_reset() {
        let mut lifecycle =
            RuntimeLifecycle::new(RuntimeInstanceId::new(1), RuntimeLifecycleConfig::Demand);
        lifecycle.start().expect("start lifecycle");
        lifecycle.admit_demand_step().expect("admit one step");
        let before_fault = lifecycle.readout();
        lifecycle
            .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
            .expect("report owner fault");

        let faulted = lifecycle.readout();
        let expected_faulted = ProductHostRuntimeReadout::new(
            dev_binding(faulted),
            product_host::ProductHostRuntimeMode::Demand,
            ProductHostRuntimeState::Faulted,
        )
        .with_counters(
            before_fault.admitted_simulation_steps(),
            before_fault.admitted_presentations(),
            before_fault
                .dropped_realtime_steps()
                .min(u128::from(u64::MAX)) as u64,
            before_fault.clock_regressions(),
        )
        .with_clock(None, None)
        .with_fault(ProductHostRuntimeFault::OwnerReported);
        assert_eq!(dev_readout(faulted), expected_faulted);

        lifecycle.restart().expect("restart lifecycle");
        let restarted = lifecycle.readout();
        let expected_restarted = ProductHostRuntimeReadout::new(
            dev_binding(restarted),
            product_host::ProductHostRuntimeMode::Demand,
            ProductHostRuntimeState::Running,
        )
        .with_counters(0, 0, 0, 0)
        .with_clock(None, None);
        assert_eq!(dev_readout(restarted), expected_restarted);
    }

    #[test]
    fn persistence_root_is_explicit_and_prepared_before_product_creation() {
        let root = std::env::temp_dir().join(format!(
            "rusty-engine-persistence-root-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        assert!(!root.exists());

        let prepared = prepare_persistence_root(Some(&root)).expect("explicit root");
        assert_eq!(prepared.as_deref(), Some(root.as_path()));
        assert!(root.is_dir());

        assert!(prepare_persistence_root(None)
            .expect("optional root")
            .is_none());
        let relative = Path::new("relative-persistence-root");
        let error = prepare_persistence_root(Some(relative)).expect_err("relative root");
        assert_eq!(error.code(), "CSHARP_PERSISTENCE_ROOT");

        fs::remove_dir_all(root).expect("remove fixture root");
    }

    #[test]
    fn lifecycle_and_input_fault_codes_preserve_variant_identity() {
        let clock_regression = runtime_lifecycle::RuntimeLifecycleError::ClockRegression {
            previous: HostMonotonicTime::from_nanoseconds(2),
            observed: HostMonotonicTime::from_nanoseconds(1),
        };
        assert_eq!(
            lifecycle_runtime_error(clock_regression).code(),
            "CSHARP_LIFECYCLE_CLOCK_REGRESSION"
        );
        assert_eq!(
            lifecycle_runtime_error(runtime_lifecycle::RuntimeLifecycleError::CounterExhausted)
                .code(),
            "CSHARP_LIFECYCLE_COUNTER_EXHAUSTED"
        );
        assert_eq!(
            input_runtime_error(runtime_input::RuntimeInputError::BindingMismatch).code(),
            "CSHARP_INPUT_BINDING_MISMATCH"
        );
    }
}
