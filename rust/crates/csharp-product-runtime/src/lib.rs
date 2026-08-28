//! Deliberately permissive, experimental loader for one trusted NativeAOT C# product.
//!
//! This is a walking trial, not a product plugin framework or a compatibility
//! promise. The product is first-party trusted code. This adapter owns only the
//! fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its gameplay state and orchestration.

pub use csharp_engine_abi::*;

use std::{
    collections::BTreeSet,
    ffi::c_void,
    fs,
    path::{Component, Path, PathBuf},
    ptr,
    sync::Arc,
};

use csharp_engine_services::{
    CsharpAppearanceCatalog, CsharpEngineServicesError, CsharpRenderResource,
    CsharpRenderResourceKind, EngineServiceSet,
};
use libloading::Library;
use product_dev_host::{
    CanonicalU64, ProductDevControlOperation, ProductDevInputBatch, ProductDevInputResult,
    ProductDevLifecycleOperation, ProductDevOperationKind, ProductDevOperationResult,
    ProductDevRendererResource, ProductDevRuntime, ProductDevRuntimeBinding,
    ProductDevRuntimeError, ProductDevRuntimeFault, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeState,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};
use product_model::{InputAxis, InputEdge, IntentValueKind};
use runtime_input::{
    AxisValue, CompiledInputMappings, DirectInputIntentDescriptor, InputClearReason, InputContext,
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
const RUNTIME_INSTANCE_ID: u64 = 1;
const STANDARD_REALTIME_HZ: u32 = 60;
const STANDARD_MAX_CATCH_UP_STEPS: u32 = 4;
const STANDARD_REALTIME_EXERCISE_ADMISSION_NS: u64 = 16_666_667;
// The Engine-owned Product Browser Host uses this same default when its
// generated bundle does not supply an input-context override. Keeping the
// standard native runtime on that typed host default lets generated physical
// input reach RuntimeInputLane without product-local bundle edits.
const STANDARD_INPUT_CONTEXT: &str = "gameplay.default";
const REALTIME_TURN_KIND: NativeProductTurnKind = NativeProductTurnKind::Realtime;
const DEMAND_TURN_KIND: NativeProductTurnKind = NativeProductTurnKind::Demand;
const EXTERNAL_TURN_KIND: NativeProductTurnKind = NativeProductTurnKind::External;
// These are host admission bounds, before the immutable Content service owns
// references. The per-file limit matches the Engine renderer resource limit;
// the aggregate limit matches the existing product persistence payload limit.
const MAX_CONTENT_FILES: usize = 8_192;
const MAX_CONTENT_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CONTENT_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct CsharpProductRuntimeError {
    code: &'static str,
    detail: String,
}

/// Explicit standard-runtime configuration. Lifecycle selection, direct input
/// descriptors, and physical mappings are Engine-owned host configuration,
/// not product policy.
#[derive(Debug, Clone)]
pub struct CsharpProductRuntimeConfig {
    lifecycle: RuntimeLifecycleConfig,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    physical_mappings: Vec<RuntimeInputMapping>,
    /// Optional host-selected application root for opaque product state.
    /// Products choose only relative scopes beneath this root.
    persistence_root: Option<PathBuf>,
    /// Optional host-selected root for admitted content-store generations.
    /// Omitting it leaves this distinct service unavailable.
    content_store_root: Option<PathBuf>,
}

impl CsharpProductRuntimeConfig {
    pub fn new(
        lifecycle: RuntimeLifecycleConfig,
        direct_intents: Vec<DirectInputIntentDescriptor>,
    ) -> Self {
        Self {
            lifecycle,
            direct_intents,
            physical_mappings: Vec::new(),
            persistence_root: None,
            content_store_root: None,
        }
    }

    /// Adds typed standard-runtime physical mappings to create-time host
    /// configuration. The product receives a copied descriptor; it does not
    /// own or mutate the runtime lane's mapping evaluation.
    pub fn with_physical_mappings(mut self, mappings: Vec<RuntimeInputMapping>) -> Self {
        self.physical_mappings = mappings;
        self
    }

    /// Selects the explicit host-owned root used by product persistence.
    /// There is no implicit filesystem root; omitting this option leaves the
    /// persistence service unconfigured for products that do not need it.
    pub fn with_persistence_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.persistence_root = Some(root.into());
        self
    }

    /// Selects the explicit host-owned root for content-store execution.
    pub fn with_content_store_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.content_store_root = Some(root.into());
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

impl From<CsharpProductRuntimeError> for ProductDevRuntimeError {
    fn from(error: CsharpProductRuntimeError) -> Self {
        ProductDevRuntimeError::new(error.code, error.detail)
            .expect("fixed bounded NativeAOT error")
    }
}

struct LoadedProductApi {
    // NativeAOT initializes process-wide managed runtime support. It does not
    // provide a safe shared-library unload contract, so a successfully created
    // product keeps its library mapped until process exit after destroy.
    library: Option<Library>,
    create: NativeProductCreate,
    start: NativeProductAction,
    turn: NativeProductTurn,
    pause: NativeProductAction,
    resume: NativeProductAction,
    restart: NativeProductAction,
    shutdown: NativeProductAction,
    destroy: NativeProductDestroy,
}

impl LoadedProductApi {
    fn load(path: &Path) -> Result<Self, CsharpProductRuntimeError> {
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
        let bind: NativeProductBind = unsafe { symbol(&library, b"rusty_product_bind\0") }?;
        let mut product = NativeProductApi::default();
        // SAFETY: `product` is a writable generated table with exact C layout.
        let status = unsafe { bind(&mut product) };
        checked_status(status, "bind")?;
        Ok(Self {
            create: required_function(product.create, "create")?,
            start: required_function(product.start, "start")?,
            turn: required_function(product.turn, "turn")?,
            pause: required_function(product.pause, "pause")?,
            resume: required_function(product.resume, "resume")?,
            restart: required_function(product.restart, "restart")?,
            shutdown: required_function(product.shutdown, "shutdown")?,
            destroy: required_function(product.destroy, "destroy")?,
            library: Some(library),
        })
    }
}

fn required_function<T>(function: Option<T>, name: &str) -> Result<T, CsharpProductRuntimeError> {
    function.ok_or_else(|| {
        CsharpProductRuntimeError::new(
            "CSHARP_REQUIRED_FUNCTION",
            format!("C# product did not bind required function `{name}`"),
        )
    })
}

/// A loaded trusted C# product adapted to the existing local browser host.
pub struct CsharpProductRuntime {
    api: LoadedProductApi,
    handle: *mut c_void,
    lifecycle: RuntimeLifecycle,
    input_lane: RuntimeInputLane,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    pending_inputs: Vec<NativeInputOwned>,
    services: Box<EngineServiceSet>,
    initial_outputs: Vec<ProductDevRuntimeOutput>,
    render_resources: Vec<ProductDevRendererResource>,
    shutdown_called: bool,
}

// The development host serializes every call with one mutex. The native handle
// has no ambient access from Rust and is destroyed before the process-lifetime
// NativeAOT library mapping is retained for process exit.
unsafe impl Send for CsharpProductRuntime {}

/// Callback state remains Engine-owned for the complete NativeAOT runtime lifetime.
/// A C# call only borrows its value arena; Rust copies it into envelopes and commits
impl CsharpProductRuntime {
    /// Renderer resources selected by product creation before host startup.
    pub fn render_resources(&self) -> &[ProductDevRendererResource] {
        &self.render_resources
    }

    /// Loads one C# library and creates its authoritative product state.
    pub fn load(
        library_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let content = CsharpProductContent::admit(content_root)?;
        Self::load_admitted(library_path, content, config)
    }

    /// Loads one product from content already read and admitted before host startup.
    pub fn load_admitted(
        library_path: impl AsRef<Path>,
        content: CsharpProductContent,
        config: CsharpProductRuntimeConfig,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let persistence_root = prepare_persistence_root(config.persistence_root.as_deref())?;
        let content_store_root = prepare_content_store_root(config.content_store_root.as_deref())?;
        let input_mappings = CompiledInputMappings::standard(
            config.direct_intents.clone(),
            config.physical_mappings.clone(),
        )
        .map_err(input_error)?;
        let lifecycle = RuntimeLifecycle::new(
            RuntimeInstanceId::new(RUNTIME_INSTANCE_ID),
            config.lifecycle,
        );
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
            context: input_context.as_ptr(),
            context_len: input_context.len(),
            direct_intents: native_input_descriptors.as_ptr(),
            direct_intents_len: native_input_descriptors.len(),
            physical_mappings: native_physical_mappings.as_ptr(),
            physical_mappings_len: native_physical_mappings.len(),
        };
        let api = LoadedProductApi::load(library_path.as_ref())?;
        let CsharpProductContent {
            files: content,
            appearance_catalog,
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
        let mut services = Box::new(EngineServiceSet::new(
            appearance_catalog,
            content_resources,
            persistence_root,
            content_store_root,
        )?);
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
        services.begin_call(ui_binding(&lifecycle));
        match call_create(&api, &args, &mut handle) {
            Ok(()) => {}
            Err(error) => {
                services.discard_call();
                if !handle.is_null() {
                    // SAFETY: a failing create may still have returned an owned
                    // handle; releasing it is part of the fixed ownership ABI.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        }
        let staged = match services
            .take_call()
            .map_err(CsharpProductRuntimeError::from)
        {
            Ok(staged) => staged,
            Err(error) => {
                services.discard_call();
                if !handle.is_null() {
                    // SAFETY: successful create produced this owned product handle.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        };
        if handle.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CREATE_HANDLE",
                "rusty_product_create succeeded but returned a null product handle",
            ));
        }
        let initial_outputs = service_outputs(services.outputs(&staged))?;
        services.commit_call(staged);
        services.seal_resource_selection();
        let render_resources = match services
            .render_resources()
            .iter()
            .map(admit_renderer_resource)
            .collect()
        {
            Ok(resources) => resources,
            Err(error) => {
                // SAFETY: create returned this owned handle and admission
                // failed before the runtime could retain it.
                unsafe { (api.destroy)(handle) };
                return Err(error);
            }
        };
        let input_lane =
            RuntimeInputLane::new(input_mappings, initial_binding, standard_input_context());
        Ok(Self {
            api,
            handle,
            lifecycle,
            input_lane,
            direct_intents: config.direct_intents,
            pending_inputs: Vec::new(),
            services,
            initial_outputs,
            render_resources,
            shutdown_called: false,
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

    /// Exercises the selected lifecycle mode plus its rejected neighbouring
    /// operation. Rejection happens before the NativeAOT product turn, so its
    /// pending input and lifecycle counters remain unchanged.
    pub fn exercise_turns(&mut self) -> Result<(), CsharpProductRuntimeError> {
        self.start_for_exercise()?;
        let started_binding = input_binding(&self.lifecycle);
        self.exercise_ui_projection_binding(started_binding)?;
        self.input(ProductDevInputBatch::new(vec![key_press(
            started_binding,
            1,
        )]))
        .map_err(exercise_runtime_error)?;
        self.control(
            ProductDevControlOperation::Replace,
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
            .input(ProductDevInputBatch::new(vec![input_clear(
                started_binding,
                2,
            )]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_STALE_INPUT",
                "stale input was admitted after a control revision",
            ));
        }
        self.exercise_ui_projection_binding(replaced_binding)?;
        self.input(ProductDevInputBatch::new(vec![key_press(
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
            ProductDevControlOperation::Release,
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
                ProductDevControlOperation::Release,
                dev_binding_from_input(replaced_binding),
            )
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_STALE_CONTROL",
                "stale control release was admitted after a control revision",
            ));
        }
        self.input(ProductDevInputBatch::new(vec![input_clear(
            released_binding,
            1,
        )]))
        .map_err(exercise_runtime_error)?;
        self.exercise_physical_mapping(released_binding)?;
        self.exercise_direct_intent(released_binding)?;
        self.exercise_selected_mode()?;
        self.exercise_pause_resume()?;
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
        assert_ui_projection_binding(&outputs, expected)
    }

    fn exercise_pause_resume(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let running_binding = self.binding();
        self.lifecycle_with_binding(ProductDevLifecycleOperation::Pause, Some(running_binding))
            .map_err(exercise_runtime_error)?;
        if self.lifecycle.state() != RuntimeState::Paused {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_PAUSE",
                "pause did not leave the Rust lifecycle paused",
            ));
        }
        let paused_binding = self.binding();
        let paused_operation = self.advance_realtime(CanonicalU64::new(0));
        if paused_operation.is_ok() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_PAUSE",
                "a paused lifecycle admitted realtime work",
            ));
        }
        self.lifecycle_with_binding(ProductDevLifecycleOperation::Resume, Some(paused_binding))
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
        self.input(ProductDevInputBatch::new(vec![fault_key_press(
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
                "product fault request did not preserve the completed turn counters and typed fault state",
            ));
        }
        if self
            .input(ProductDevInputBatch::new(vec![key_press(
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
                ProductDevLifecycleOperation::Restart,
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
            .input(ProductDevInputBatch::new(vec![key_press(
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
        self.input(ProductDevInputBatch::new(vec![key_press(
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
        self.input(ProductDevInputBatch::new(vec![key_release(
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
        if self.input(ProductDevInputBatch::new(vec![stale])).is_ok() {
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
            .input(ProductDevInputBatch::new(vec![unmapped]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_UNMAPPED_DIRECT_INTENT",
                "unmapped payload direct intent was admitted",
            ));
        }
        let mismatched = payload_intent(
            current_binding,
            next_sequence.checked_add(1).ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_DIRECT_INTENT",
                    "direct-intent exercise sequence overflowed",
                )
            })?,
            descriptor.id(),
            "runtime.wrong.contract",
        )?;
        if self
            .input(ProductDevInputBatch::new(vec![mismatched]))
            .is_ok()
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_MISMATCHED_DIRECT_INTENT",
                "payload direct intent with a mismatched contract was admitted",
            ));
        }
        let admitted = direct_intent(
            current_binding,
            next_sequence.checked_add(2).ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_EXERCISE_DIRECT_INTENT",
                    "direct-intent exercise sequence overflowed",
                )
            })?,
            &descriptor,
        )?;
        self.input(ProductDevInputBatch::new(vec![admitted]))
            .map_err(exercise_runtime_error)?;
        let native = self.pending_inputs.last().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_DIRECT_INTENT",
                "admitted direct intent did not reach the ProductInputEvent conversion queue",
            )
        })?;
        if native.label != descriptor.id().as_bytes()
            || native.kind != direct_intent_native_kind(descriptor.value_kind())
            || native.payload_contract != contract.as_bytes()
            || native.payload_data != br#"{"exercise":true}"#
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_DIRECT_INTENT",
                "direct intent was not converted to the configured safe ProductInputEvent shape",
            ));
        }
        Ok(())
    }

    fn exercise_selected_mode(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let selected_mode = self.lifecycle.mode();
        let readout_mode = self.readout().mode();
        let expected_readout_mode = match selected_mode {
            RuntimeMode::Realtime => product_dev_host::ProductDevRuntimeMode::Realtime,
            RuntimeMode::Demand => product_dev_host::ProductDevRuntimeMode::Demand,
            RuntimeMode::External => product_dev_host::ProductDevRuntimeMode::External,
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
                self.advance_realtime(CanonicalU64::new(
                    baseline
                        .checked_add(STANDARD_REALTIME_EXERCISE_ADMISSION_NS)
                        .ok_or_else(|| {
                            CsharpProductRuntimeError::new(
                                "CSHARP_EXERCISE_REALTIME",
                                "realtime exercise observation overflowed",
                            )
                        })?,
                ))
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
                    CanonicalU64::new(admitted_before.checked_add(2).ok_or_else(|| {
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
                .checked_add(1)
                .expect("successful lifecycle admission cannot overflow")
        {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_ADMISSION",
                "selected lifecycle mode did not admit exactly one Product.Game turn",
            ));
        }
        Ok(())
    }

    fn turn(
        &mut self,
        facts: NativeProductUpdateFacts,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        self.services.begin_call(ui_binding(&self.lifecycle));
        let request = match call_turn(
            &self.api,
            self.handle,
            NativeTurnArgs {
                facts,
                events: events.as_ptr(),
                event_count: events.len(),
            },
        ) {
            Ok(request) => request,
            Err(error) => {
                self.services.discard_call();
                return Err(error);
            }
        };
        let staged = match self.services.take_call() {
            Ok(staged) => staged,
            Err(error) => {
                self.services.discard_call();
                return Err(error.into());
            }
        };
        let mut outputs = match service_outputs(self.services.outputs(&staged)) {
            Ok(outputs) => outputs,
            Err(error) => {
                self.services.discard_call();
                return Err(error);
            }
        };
        // Clear only after staged Engine output has been converted. A failed
        // conversion must preserve the input for the caller's failure path.
        self.pending_inputs.clear();
        self.services.commit_call(staged);

        if request == NativeProductTurnRequest::ReportFault {
            // Product requests are intentionally deferred until the completed
            // Engine service turn is committed. This is a typed lifecycle
            // signal, not a reentrant service call or a general event bus.
            self.lifecycle
                .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
                .map_err(lifecycle_error)?;
            self.rebind_input(InputClearReason::ControlRevisionChange)?;
            let binding = self.binding();
            outputs.push(ProductDevRuntimeOutput::binding(binding));
            outputs.push(ProductDevRuntimeOutput::complete_baseline(binding));
        }
        Ok(outputs)
    }

    fn turn_admitted(
        &mut self,
        kind: NativeProductTurnKind,
        observed_host_time_nanoseconds: Option<u64>,
        admission: runtime_lifecycle::SimulationAdmission,
        dropped_step_count: u128,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        // Realtime catch-up remains one Product.Game turn per host
        // observation. Correlate that turn with the last lifecycle-admitted
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
            .filter(|envelope| {
                matches!(
                    envelope.provenance(),
                    runtime_input::IntentProvenance::Physical { .. }
                )
            })
            .map(|envelope| native_intent_event(envelope, &context))
            .collect::<Vec<_>>();
        self.pending_inputs.extend(mapped);
        let facts = update_facts(
            &self.lifecycle,
            kind,
            observed_host_time_nanoseconds,
            admission,
            dropped_step_count,
        )?;
        self.turn(facts)
    }

    fn action<F, T>(
        &mut self,
        action: NativeProductAction,
        operation: ProductDevOperationKind,
        transition: F,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError>
    where
        F: FnOnce(&mut RuntimeLifecycle) -> Result<T, runtime_lifecycle::RuntimeLifecycleError>,
    {
        self.services.begin_call(ui_binding(&self.lifecycle));
        match call_action(action, self.handle, operation) {
            Ok(()) => {}
            Err(error) => {
                self.services.discard_call();
                return Err(error);
            }
        }
        let mut staged = match self.services.take_call() {
            Ok(staged) => staged,
            Err(error) => {
                self.services.discard_call();
                return Err(error.into());
            }
        };
        // Convert the complete staged output before the Rust lifecycle is
        // changed. The later UI retag is infallible: it changes only the
        // already-typed runtime binding of an already validated envelope.
        if let Err(error) = service_outputs(self.services.outputs(&staged)) {
            self.services.discard_call();
            return Err(error);
        }
        if let Err(error) = transition(&mut self.lifecycle) {
            self.services.discard_call();
            return Err(lifecycle_error(error));
        }
        staged.rebind_ui_runtime(ui_binding(&self.lifecycle));
        let mut outputs = if matches!(operation, ProductDevOperationKind::Start) {
            std::mem::take(&mut self.initial_outputs)
        } else {
            Vec::new()
        };
        outputs.extend(
            service_outputs(self.services.outputs(&staged))
                .expect("rebinding typed UI identity cannot invalidate prevalidated output"),
        );
        self.services.commit_call(staged);
        Ok(outputs)
    }

    fn receipt(
        &self,
        operation: ProductDevOperationKind,
        outputs: Vec<ProductDevRuntimeOutput>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let readout = self.readout();
        let result = ProductDevOperationResult::accepted(operation, self.binding(), readout)
            .map_err(host_runtime_error)?;
        ProductDevRuntimeReceipt::new(result, outputs).map_err(host_runtime_error)
    }

    fn readout(&self) -> ProductDevRuntimeReadout {
        dev_readout(self.lifecycle.readout())
    }

    fn runtime_error(&self, error: CsharpProductRuntimeError) -> ProductDevRuntimeError {
        ProductDevRuntimeError::new(error.code(), error.detail().to_owned())
            .expect("fixed bounded NativeAOT error")
    }

    fn binding(&self) -> ProductDevRuntimeBinding {
        dev_binding(self.lifecycle.readout())
    }

    fn require_control_binding(
        &self,
        operation: ProductDevLifecycleOperation,
        binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<(), ProductDevRuntimeError> {
        if operation == ProductDevLifecycleOperation::Start
            && self.lifecycle.state() == RuntimeState::Created
            && binding.is_none_or(|value| value == self.binding())
        {
            return Ok(());
        }
        self.require_current_control_binding(binding)
    }

    fn require_current_control_binding(
        &self,
        binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<(), ProductDevRuntimeError> {
        if binding == Some(self.binding()) {
            return Ok(());
        }
        Err(ProductDevRuntimeError::new(
            "CSHARP_CONTROL_BINDING",
            "lifecycle control does not name the current runtime binding",
        )
        .expect("fixed control-binding diagnostic"))
    }

    /// Checks restart's lifecycle state without advancing any Engine-owned
    /// counter. The product callback must not run for an impossible host
    /// transition, because the Rust lifecycle remains the authority that
    /// decides whether a new generation can be admitted.
    fn require_restart_state(&self) -> Result<(), ProductDevRuntimeError> {
        if matches!(
            self.lifecycle.state(),
            RuntimeState::Running | RuntimeState::Paused | RuntimeState::Faulted
        ) {
            return Ok(());
        }
        Err(ProductDevRuntimeError::new(
            "CSHARP_LIFECYCLE_ADMISSION",
            format!(
                "restart is not admitted from lifecycle state {:?}",
                self.lifecycle.state()
            ),
        )
        .expect("fixed lifecycle-state diagnostic"))
    }

    fn tag_complete_baseline(
        &self,
        mut outputs: Vec<ProductDevRuntimeOutput>,
    ) -> Vec<ProductDevRuntimeOutput> {
        let binding = self.binding();
        let mut tagged = Vec::with_capacity(outputs.len() + 2);
        tagged.push(ProductDevRuntimeOutput::binding(binding));
        tagged.append(&mut outputs);
        tagged.push(ProductDevRuntimeOutput::complete_baseline(binding));
        tagged
    }

    fn rebind_input(&mut self, reason: InputClearReason) -> Result<(), CsharpProductRuntimeError> {
        let binding = input_binding(&self.lifecycle);
        self.input_lane
            .rebind(binding, standard_input_context(), reason)
            .map_err(input_error)?;
        self.pending_inputs.clear();
        self.pending_inputs.push(clear_input_owned(binding, reason));
        Ok(())
    }

    fn start_for_exercise(&mut self) -> Result<(), CsharpProductRuntimeError> {
        let outputs = self.action(
            self.api.start,
            ProductDevOperationKind::Start,
            |lifecycle| lifecycle.start(),
        )?;
        assert_ui_projection_binding(&outputs, input_binding(&self.lifecycle))?;
        self.rebind_input(InputClearReason::Restart)
    }
}

impl ProductDevRuntime for CsharpProductRuntime {
    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.lifecycle_with_binding(operation, Some(self.binding()))
    }

    fn lifecycle_with_binding(
        &mut self,
        operation: ProductDevLifecycleOperation,
        binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.require_control_binding(operation, binding)?;
        match operation {
            ProductDevLifecycleOperation::Start => {
                let outputs = self
                    .action(
                        self.api.start,
                        ProductDevOperationKind::Start,
                        |lifecycle| lifecycle.start(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::Restart)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Start,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Pause => {
                let outputs = self
                    .action(
                        self.api.pause,
                        ProductDevOperationKind::Pause,
                        |lifecycle| lifecycle.pause(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Pause,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Resume => {
                let outputs = self
                    .action(
                        self.api.resume,
                        ProductDevOperationKind::Resume,
                        |lifecycle| lifecycle.resume(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Resume,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Restart => {
                // Keep the callback-first ordering used by the other product
                // lifecycle actions, but validate the Rust-owned state before
                // entering C#. A callback failure therefore leaves the
                // authoritative lifecycle binding and generation untouched.
                self.require_restart_state()?;
                let outputs = self
                    .action(
                        self.api.restart,
                        ProductDevOperationKind::Restart,
                        |lifecycle| lifecycle.restart(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.rebind_input(InputClearReason::Restart)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Restart,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::ReportFault => {
                // Fault reporting is a host control, not a reentrant product
                // callback. RuntimeLifecycle preserves its counters while
                // advancing the control revision and recording the typed
                // owner fault.
                self.lifecycle
                    .report_fault(runtime_lifecycle::RuntimeFault::OwnerReported)
                    .map_err(lifecycle_runtime_error)?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::ReportFault,
                    self.tag_complete_baseline(Vec::new()),
                )
            }
            ProductDevLifecycleOperation::Shutdown => {
                let outputs = self
                    .action(
                        self.api.shutdown,
                        ProductDevOperationKind::Shutdown,
                        |lifecycle| lifecycle.shutdown(),
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.input_lane.dispose();
                self.pending_inputs.clear();
                let receipt = self.receipt(
                    ProductDevOperationKind::Shutdown,
                    self.tag_complete_baseline(outputs),
                );
                self.shutdown_called = receipt.is_ok();
                receipt
            }
        }
    }

    fn control(
        &mut self,
        operation: ProductDevControlOperation,
        binding: ProductDevRuntimeBinding,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.require_current_control_binding(Some(binding))?;
        let lifecycle_operation = match operation {
            ProductDevControlOperation::Replace => RuntimeControlOperation::Replace,
            ProductDevControlOperation::Release => RuntimeControlOperation::Release,
        };
        self.lifecycle
            .change_control(lifecycle_operation)
            .map_err(lifecycle_runtime_error)?;
        self.rebind_input(InputClearReason::ControlRevisionChange)
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(
            operation.operation_kind(),
            self.tag_complete_baseline(Vec::new()),
        )
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError> {
        if self.lifecycle.state() != RuntimeState::Running {
            return Err(ProductDevRuntimeError::new(
                "CSHARP_INPUT_STATE",
                "input is admitted only while the standard runtime is running",
            )
            .expect("fixed input-state diagnostic"));
        }
        let native = batch
            .events()
            .iter()
            .map(|event| {
                self.input_lane
                    .ingest(event.clone())
                    .map_err(input_runtime_error)?;
                Ok(native_event(event))
            })
            .collect::<Result<Vec<_>, ProductDevRuntimeError>>()?;
        self.pending_inputs.extend(native);
        let result =
            ProductDevInputResult::accepted(batch.events().len(), self.binding(), self.readout())
                .map_err(host_runtime_error)?;
        ProductDevRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error)
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let admission = self
            .lifecycle
            .advance_realtime(HostMonotonicTime::from_nanoseconds(observed_time_ns.get()))
            .map_err(lifecycle_runtime_error)?;
        let outputs = match admission.simulation() {
            // The lifecycle owns admission and its readout counters. Runtime
            // Input snapshots once with the last admitted phase token; the
            // product receives one turn per accepted host observation while
            // retaining the host observation as its realtime timing value.
            Some(simulation) => self
                .turn_admitted(
                    REALTIME_TURN_KIND,
                    Some(observed_time_ns.get()),
                    simulation,
                    admission.dropped_steps(),
                )
                .map_err(|error| self.runtime_error(error))?,
            None => Vec::new(),
        };
        self.receipt(ProductDevOperationKind::AdvanceRealtime, outputs)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let admission = self
            .lifecycle
            .admit_demand_step()
            .map_err(lifecycle_runtime_error)?;
        let outputs = self
            .turn_admitted(DEMAND_TURN_KIND, None, admission, 0)
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitDemandStep, outputs)
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let admission = self
            .lifecycle
            .admit_external_step(ExternalStep::new(step.get()))
            .map_err(lifecycle_runtime_error)?;
        let outputs = self
            .turn_admitted(EXTERNAL_TURN_KIND, None, admission, 0)
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitExternalStep, outputs)
    }

    fn complete_timeline(
        &mut self,
        _completion: ProductDevTimelineCompletion,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>
    {
        Err(ProductDevRuntimeError::new(
            "CSHARP_TIMELINE_UNSUPPORTED",
            "the NativeAOT walking trial has no timeline bridge",
        )
        .expect("fixed error"))
    }
}

impl Drop for CsharpProductRuntime {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        if !self.shutdown_called {
            // SAFETY: `handle` was produced by this retained library, and no
            // other Rust path destroys it. Native exceptions must not cross ABI.
            let _ = unsafe { (self.api.shutdown)(self.handle) };
        }
        // SAFETY: destroy runs exactly once before the `Library` field drops.
        unsafe { (self.api.destroy)(self.handle) };
        self.handle = ptr::null_mut();
        // A NativeAOT shared library may retain runtime worker infrastructure
        // beyond its exported destroy function. Process-lifetime mapping keeps
        // Drop safe while preserving the required product destroy ordering.
        if let Some(library) = self.api.library.take() {
            std::mem::forget(library);
        }
    }
}

fn dev_binding(readout: RuntimeLifecycleReadout) -> ProductDevRuntimeBinding {
    ProductDevRuntimeBinding {
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

fn dev_binding_from_input(binding: RuntimeInputBinding) -> ProductDevRuntimeBinding {
    ProductDevRuntimeBinding {
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
            code: product_model::KeyboardControl::KeyW,
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
            code: product_model::KeyboardControl::KeyF,
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
            code: product_model::KeyboardControl::KeyW,
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

fn direct_intent_native_kind(value_kind: IntentValueKind) -> NativeInputEventKind {
    match value_kind {
        IntentValueKind::Digital => NativeInputEventKind::DirectDigital,
        IntentValueKind::Axis => NativeInputEventKind::DirectAxis,
        IntentValueKind::ProductPayload => NativeInputEventKind::DirectProductPayload,
    }
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

fn dev_readout(readout: RuntimeLifecycleReadout) -> ProductDevRuntimeReadout {
    let mode = match readout.mode() {
        RuntimeMode::Realtime => product_dev_host::ProductDevRuntimeMode::Realtime,
        RuntimeMode::Demand => product_dev_host::ProductDevRuntimeMode::Demand,
        RuntimeMode::External => product_dev_host::ProductDevRuntimeMode::External,
    };
    let state = match readout.state() {
        RuntimeState::Created => ProductDevRuntimeState::Created,
        RuntimeState::Running => ProductDevRuntimeState::Running,
        RuntimeState::Paused => ProductDevRuntimeState::Paused,
        RuntimeState::Faulted => ProductDevRuntimeState::Faulted,
        RuntimeState::Shutdown => ProductDevRuntimeState::Shutdown,
    };
    let mut projected = ProductDevRuntimeReadout::new(dev_binding(readout), mode, state)
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
            runtime_lifecycle::RuntimeFault::OwnerReported => ProductDevRuntimeFault::OwnerReported,
            runtime_lifecycle::RuntimeFault::CounterExhausted => {
                ProductDevRuntimeFault::CounterExhausted
            }
        });
    }
    projected
}

fn update_facts(
    lifecycle: &RuntimeLifecycle,
    mode: NativeProductTurnKind,
    observed_host_time_nanoseconds: Option<u64>,
    admission: runtime_lifecycle::SimulationAdmission,
    dropped_step_count: u128,
) -> Result<NativeProductUpdateFacts, CsharpProductRuntimeError> {
    let readout = lifecycle.readout();
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
        simulation_step: admission.first_step().value(),
        fixed_step_hz,
        admitted_step_count: admission.step_count(),
        dropped_step_count,
        fixed_delta_seconds,
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

fn prepare_content_store_root(
    root: Option<&Path>,
) -> Result<Option<PathBuf>, CsharpProductRuntimeError> {
    let Some(root) = root else {
        return Ok(None);
    };
    if root.as_os_str().is_empty() || !root.is_absolute() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_STORE_ROOT",
            "content store root must be an explicit absolute host path",
        ));
    }
    fs::create_dir_all(root).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_STORE_ROOT",
            format!(
                "could not create content store root {}: {error}",
                root.display()
            ),
        )
    })?;
    if !root.is_dir() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_STORE_ROOT",
            format!("content store root {} is not a directory", root.display()),
        ));
    }
    Ok(Some(root.to_path_buf()))
}

fn input_runtime_error(error: runtime_input::RuntimeInputError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new("CSHARP_INPUT_ADMISSION", error.to_string())
        .expect("bounded input admission diagnostic")
}

fn lifecycle_error(error: runtime_lifecycle::RuntimeLifecycleError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new("CSHARP_LIFECYCLE_ADMISSION", error.to_string())
}

fn lifecycle_runtime_error(
    error: runtime_lifecycle::RuntimeLifecycleError,
) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new("CSHARP_LIFECYCLE_ADMISSION", error.to_string())
        .expect("bounded lifecycle admission diagnostic")
}

fn exercise_runtime_error(error: ProductDevRuntimeError) -> CsharpProductRuntimeError {
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
    // SAFETY: fixed ABI pointers are valid for the duration of this call.
    let status = unsafe { (api.create)(args, handle) };
    checked_status(status, "create")
}

fn call_action(
    action: NativeProductAction,
    handle: *mut c_void,
    operation: ProductDevOperationKind,
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: `handle` is retained by the runtime.
    let status = unsafe { action(handle) };
    checked_status(status, operation_name(operation))
}

fn call_turn(
    api: &LoadedProductApi,
    handle: *mut c_void,
    args: NativeTurnArgs,
) -> Result<NativeProductTurnRequest, CsharpProductRuntimeError> {
    let mut request = NativeProductTurnRequest::None;
    // SAFETY: event label pointers borrow local strings that remain alive for
    // the call; the C# product is required to copy anything it retains.
    let status = unsafe { (api.turn)(handle, &args, &mut request) };
    checked_status(status, "turn")?;
    Ok(request)
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

fn operation_name(operation: ProductDevOperationKind) -> &'static str {
    match operation {
        ProductDevOperationKind::Start => "start",
        ProductDevOperationKind::Pause => "pause",
        ProductDevOperationKind::Resume => "resume",
        ProductDevOperationKind::Restart => "restart",
        ProductDevOperationKind::Shutdown => "shutdown",
        _ => "operation",
    }
}

#[derive(Debug)]
struct ContentFile {
    path: Vec<u8>,
    bytes: Arc<[u8]>,
}

#[derive(Debug)]
struct ContentCandidate {
    host_path: PathBuf,
    product_path: Vec<u8>,
    byte_length: u64,
}

#[derive(Debug, Clone, Copy)]
struct ContentAdmissionLimits {
    max_files: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
}

const CONTENT_ADMISSION_LIMITS: ContentAdmissionLimits = ContentAdmissionLimits {
    max_files: MAX_CONTENT_FILES,
    max_file_bytes: MAX_CONTENT_FILE_BYTES,
    max_total_bytes: MAX_CONTENT_TOTAL_BYTES,
};

#[derive(Debug, Default)]
struct ContentAdmissionQuota {
    files: usize,
    total_bytes: u64,
}

impl ContentAdmissionQuota {
    fn admit(
        &mut self,
        path: &[u8],
        byte_length: u64,
        limits: ContentAdmissionLimits,
    ) -> Result<(), CsharpProductRuntimeError> {
        let files = self.files.checked_add(1).ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_LIMIT",
                "content file count overflowed its admission limit",
            )
        })?;
        if files > limits.max_files {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_LIMIT",
                format!(
                    "content contains more than {} files while admitting {}",
                    limits.max_files,
                    display_content_path(path)
                ),
            ));
        }
        if byte_length > limits.max_file_bytes {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_LIMIT",
                format!(
                    "content file {} has {} bytes, exceeding the {} byte limit",
                    display_content_path(path),
                    byte_length,
                    limits.max_file_bytes
                ),
            ));
        }
        let total_bytes = self.total_bytes.checked_add(byte_length).ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_LIMIT",
                "content byte total overflowed its admission limit",
            )
        })?;
        if total_bytes > limits.max_total_bytes {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_LIMIT",
                format!(
                    "content bytes total {total_bytes} exceeds the {} byte limit while admitting {}",
                    limits.max_total_bytes,
                    display_content_path(path)
                ),
            ));
        }
        self.files = files;
        self.total_bytes = total_bytes;
        Ok(())
    }
}

/// Exact product content collected once before the native runtime and immutable
/// browser bundle are constructed. Renderer bytes stay inert until C# selects a
/// supported resource through the generated appearance API during `Create`.
pub struct CsharpProductContent {
    files: Vec<ContentFile>,
    appearance_catalog: CsharpAppearanceCatalog,
}

impl CsharpProductContent {
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
        let candidates = discover_content(root, CONTENT_ADMISSION_LIMITS)?;
        let mut files = Vec::with_capacity(candidates.len());
        // The metadata pass establishes the advertised bounds before immutable
        // content is retained. Recheck observed bytes after each read: a
        // concurrent host change is rejected rather than retained over quota.
        let mut observed_quota = ContentAdmissionQuota::default();
        for candidate in candidates {
            let bytes = fs::read(&candidate.host_path).map_err(|error| {
                CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
            })?;
            observed_quota.admit(
                &candidate.product_path,
                u64::try_from(bytes.len()).map_err(|_| {
                    CsharpProductRuntimeError::new(
                        "CSHARP_CONTENT_LIMIT",
                        "content file length cannot be represented for admission",
                    )
                })?,
                CONTENT_ADMISSION_LIMITS,
            )?;
            files.push(ContentFile {
                path: candidate.product_path,
                bytes: Arc::from(bytes),
            });
        }
        let appearance_catalog = files
            .iter()
            .find(|file| file.path == b"runtime-appearances.json")
            .map(|file| file.bytes.as_ref());
        let appearance_catalog =
            csharp_engine_services::parse_runtime_appearance_catalog(appearance_catalog)?;
        Ok(Self {
            files,
            appearance_catalog,
        })
    }
}

fn discover_content(
    root: &Path,
    limits: ContentAdmissionLimits,
) -> Result<Vec<ContentCandidate>, CsharpProductRuntimeError> {
    let mut candidates = Vec::new();
    discover_content_inner(root, root, limits, &mut candidates)?;
    candidates.sort_by(|left, right| left.product_path.cmp(&right.product_path));

    let mut paths = BTreeSet::new();
    let mut quota = ContentAdmissionQuota::default();
    for candidate in &candidates {
        if !paths.insert(candidate.product_path.as_slice()) {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_PATH",
                format!(
                    "multiple host entries normalize to the product content path {}",
                    display_content_path(&candidate.product_path)
                ),
            ));
        }
        quota.admit(&candidate.product_path, candidate.byte_length, limits)?;
    }
    Ok(candidates)
}

fn discover_content_inner(
    root: &Path,
    directory: &Path,
    limits: ContentAdmissionLimits,
    candidates: &mut Vec<ContentCandidate>,
) -> Result<(), CsharpProductRuntimeError> {
    for entry in fs::read_dir(directory)
        .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string()))?
    {
        let entry = entry.map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
        })?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_ENTRY",
                format!("content traversal rejects symlink: {}", path.display()),
            ));
        }
        if file_type.is_dir() {
            discover_content_inner(root, &path, limits, candidates)?;
        } else if file_type.is_file() {
            if candidates.len() == limits.max_files {
                return Err(CsharpProductRuntimeError::new(
                    "CSHARP_CONTENT_LIMIT",
                    format!("content contains more than {} files", limits.max_files),
                ));
            }
            candidates.push(ContentCandidate {
                product_path: canonical_content_path(root, &path)?,
                host_path: path,
                byte_length: metadata.len(),
            });
        } else {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_ENTRY",
                format!(
                    "content traversal requires regular files: {}",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn canonical_content_path(root: &Path, path: &Path) -> Result<Vec<u8>, CsharpProductRuntimeError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_PATH",
            format!("content path escaped its root: {}", path.display()),
        )
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_PATH",
                format!(
                    "content path must be product-relative and normalized: {}",
                    path.display()
                ),
            ));
        };
        let part = part.to_str().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_PATH",
                format!("content path must be valid UTF-8: {}", path.display()),
            )
        })?;
        if part.is_empty() || part.contains('\\') {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_PATH",
                format!(
                    "content path must use canonical forward-slash components: {}",
                    path.display()
                ),
            ));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_PATH",
            format!("content path must be product-relative: {}", path.display()),
        ));
    }
    Ok(parts.join("/").into_bytes())
}

fn display_content_path(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
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
                runtime_input::RuntimeInputFact::PointerButton { button, edge } => (
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
                    0.0,
                    0.0,
                    format!("{button:?}"),
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
    let mapping_id = match envelope.provenance() {
        runtime_input::IntentProvenance::Physical { mapping_id } => mapping_id,
        runtime_input::IntentProvenance::DirectUi => "",
    };
    let (kind, value_kind, x, payload_contract, payload_data) = match envelope.value() {
        RuntimeIntentValue::Digital { active } => (
            NativeInputEventKind::MappedDigital,
            NativeInputValueKind::Digital,
            if active { 1.0 } else { 0.0 },
            Vec::new(),
            Vec::new(),
        ),
        RuntimeIntentValue::Axis { value } => (
            NativeInputEventKind::MappedAxis,
            NativeInputValueKind::Axis,
            value.value(),
            Vec::new(),
            Vec::new(),
        ),
        RuntimeIntentValue::ProductPayload { payload } => (
            NativeInputEventKind::MappedProductPayload,
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
    native.provenance = NativeInputProvenance::Physical;
    native.x = x;
    native.mapping_id = mapping_id.as_bytes().to_vec();
    native.intent = envelope.intent().as_bytes().to_vec();
    native.label = native.mapping_id.clone();
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

fn keyboard_control(value: product_model::KeyboardControl) -> NativeKeyboardControl {
    match value {
        product_model::KeyboardControl::KeyA => NativeKeyboardControl::KeyA,
        product_model::KeyboardControl::KeyB => NativeKeyboardControl::KeyB,
        product_model::KeyboardControl::KeyC => NativeKeyboardControl::KeyC,
        product_model::KeyboardControl::KeyD => NativeKeyboardControl::KeyD,
        product_model::KeyboardControl::KeyE => NativeKeyboardControl::KeyE,
        product_model::KeyboardControl::KeyF => NativeKeyboardControl::KeyF,
        product_model::KeyboardControl::KeyG => NativeKeyboardControl::KeyG,
        product_model::KeyboardControl::KeyH => NativeKeyboardControl::KeyH,
        product_model::KeyboardControl::KeyI => NativeKeyboardControl::KeyI,
        product_model::KeyboardControl::KeyJ => NativeKeyboardControl::KeyJ,
        product_model::KeyboardControl::KeyK => NativeKeyboardControl::KeyK,
        product_model::KeyboardControl::KeyL => NativeKeyboardControl::KeyL,
        product_model::KeyboardControl::KeyM => NativeKeyboardControl::KeyM,
        product_model::KeyboardControl::KeyN => NativeKeyboardControl::KeyN,
        product_model::KeyboardControl::KeyO => NativeKeyboardControl::KeyO,
        product_model::KeyboardControl::KeyP => NativeKeyboardControl::KeyP,
        product_model::KeyboardControl::KeyQ => NativeKeyboardControl::KeyQ,
        product_model::KeyboardControl::KeyR => NativeKeyboardControl::KeyR,
        product_model::KeyboardControl::KeyS => NativeKeyboardControl::KeyS,
        product_model::KeyboardControl::KeyT => NativeKeyboardControl::KeyT,
        product_model::KeyboardControl::KeyU => NativeKeyboardControl::KeyU,
        product_model::KeyboardControl::KeyV => NativeKeyboardControl::KeyV,
        product_model::KeyboardControl::KeyW => NativeKeyboardControl::KeyW,
        product_model::KeyboardControl::KeyX => NativeKeyboardControl::KeyX,
        product_model::KeyboardControl::KeyY => NativeKeyboardControl::KeyY,
        product_model::KeyboardControl::KeyZ => NativeKeyboardControl::KeyZ,
        product_model::KeyboardControl::Digit0 => NativeKeyboardControl::Digit0,
        product_model::KeyboardControl::Digit1 => NativeKeyboardControl::Digit1,
        product_model::KeyboardControl::Digit2 => NativeKeyboardControl::Digit2,
        product_model::KeyboardControl::Digit3 => NativeKeyboardControl::Digit3,
        product_model::KeyboardControl::Digit4 => NativeKeyboardControl::Digit4,
        product_model::KeyboardControl::Digit5 => NativeKeyboardControl::Digit5,
        product_model::KeyboardControl::Digit6 => NativeKeyboardControl::Digit6,
        product_model::KeyboardControl::Digit7 => NativeKeyboardControl::Digit7,
        product_model::KeyboardControl::Digit8 => NativeKeyboardControl::Digit8,
        product_model::KeyboardControl::Digit9 => NativeKeyboardControl::Digit9,
        product_model::KeyboardControl::Space => NativeKeyboardControl::Space,
        product_model::KeyboardControl::Enter => NativeKeyboardControl::Enter,
        product_model::KeyboardControl::Escape => NativeKeyboardControl::Escape,
        product_model::KeyboardControl::ShiftLeft => NativeKeyboardControl::ShiftLeft,
        product_model::KeyboardControl::ShiftRight => NativeKeyboardControl::ShiftRight,
        product_model::KeyboardControl::ControlLeft => NativeKeyboardControl::ControlLeft,
        product_model::KeyboardControl::ControlRight => NativeKeyboardControl::ControlRight,
        product_model::KeyboardControl::AltLeft => NativeKeyboardControl::AltLeft,
        product_model::KeyboardControl::AltRight => NativeKeyboardControl::AltRight,
    }
}

fn pointer_button(value: product_model::PointerButton) -> NativePointerButton {
    match value {
        product_model::PointerButton::Primary => NativePointerButton::Primary,
        product_model::PointerButton::Secondary => NativePointerButton::Secondary,
        product_model::PointerButton::Middle => NativePointerButton::Middle,
    }
}

fn controller_button(value: product_model::ControllerButton) -> NativeControllerButton {
    match value {
        product_model::ControllerButton::Button0 => NativeControllerButton::Button0,
        product_model::ControllerButton::Button1 => NativeControllerButton::Button1,
        product_model::ControllerButton::Button2 => NativeControllerButton::Button2,
        product_model::ControllerButton::Button3 => NativeControllerButton::Button3,
        product_model::ControllerButton::Button4 => NativeControllerButton::Button4,
        product_model::ControllerButton::Button5 => NativeControllerButton::Button5,
        product_model::ControllerButton::Button6 => NativeControllerButton::Button6,
        product_model::ControllerButton::Button7 => NativeControllerButton::Button7,
        product_model::ControllerButton::Button8 => NativeControllerButton::Button8,
        product_model::ControllerButton::Button9 => NativeControllerButton::Button9,
        product_model::ControllerButton::Button10 => NativeControllerButton::Button10,
        product_model::ControllerButton::Button11 => NativeControllerButton::Button11,
        product_model::ControllerButton::Button12 => NativeControllerButton::Button12,
        product_model::ControllerButton::Button13 => NativeControllerButton::Button13,
        product_model::ControllerButton::Button14 => NativeControllerButton::Button14,
        product_model::ControllerButton::Button15 => NativeControllerButton::Button15,
    }
}

fn controller_axis(value: product_model::ControllerAxis) -> NativeControllerAxis {
    match value {
        product_model::ControllerAxis::Axis0 => NativeControllerAxis::Axis0,
        product_model::ControllerAxis::Axis1 => NativeControllerAxis::Axis1,
        product_model::ControllerAxis::Axis2 => NativeControllerAxis::Axis2,
        product_model::ControllerAxis::Axis3 => NativeControllerAxis::Axis3,
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

fn admit_renderer_resource(
    resource: &CsharpRenderResource,
) -> Result<ProductDevRendererResource, CsharpProductRuntimeError> {
    match resource.kind() {
        CsharpRenderResourceKind::Texture => {
            ProductDevRendererResource::admit_texture(resource.path(), resource.bytes().to_vec())
        }
        CsharpRenderResourceKind::Mesh => {
            ProductDevRendererResource::admit_mesh(resource.path(), resource.bytes().to_vec())
        }
        CsharpRenderResourceKind::Font => {
            ProductDevRendererResource::admit_font(resource.path(), resource.bytes().to_vec())
        }
        CsharpRenderResourceKind::Audio => {
            ProductDevRendererResource::admit_audio(resource.path(), resource.bytes().to_vec())
        }
        CsharpRenderResourceKind::AnimatedMesh => ProductDevRendererResource::admit_animated_mesh(
            resource.path(),
            resource.bytes().to_vec(),
        ),
    }
    .map_err(|error| CsharpProductRuntimeError::new(error.code(), error.detail()))
}

fn service_outputs(
    output: csharp_engine_services::CsharpEngineCallOutput,
) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
    let mut outputs = Vec::new();
    for frame in &output.frames {
        outputs.push(ProductDevRuntimeOutput::frame(frame).map_err(host_error)?);
    }
    if let Some(composition) = output.view_composition.as_ref() {
        outputs.push(ProductDevRuntimeOutput::view_composition(composition).map_err(host_error)?);
    }
    for projection in &output.ui {
        outputs.push(ProductDevRuntimeOutput::ui_projection(projection).map_err(host_error)?);
    }
    for frame in &output.presentation {
        outputs.push(ProductDevRuntimeOutput::presentation(frame).map_err(host_error)?);
    }
    Ok(outputs)
}

fn assert_ui_projection_binding(
    outputs: &[ProductDevRuntimeOutput],
    expected: RuntimeInputBinding,
) -> Result<(), CsharpProductRuntimeError> {
    let expected_instance = expected.instance_id().value().to_string();
    let expected_generation = expected.generation().value().to_string();
    let expected_revision = expected.control_revision().value().to_string();
    let mut found = false;
    for output in outputs {
        let encoded = serde_json::to_value(output).map_err(|error| {
            CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_UI_BINDING",
                format!("could not inspect UI projection output: {error}"),
            )
        })?;
        if encoded.get("kind").and_then(serde_json::Value::as_str) != Some("ui-projection") {
            continue;
        }
        found = true;
        let runtime = encoded
            .get("envelope")
            .and_then(|envelope| envelope.get("runtime"));
        let matches_expected = runtime.is_some_and(|runtime| {
            runtime
                .get("instanceId")
                .and_then(serde_json::Value::as_str)
                == Some(expected_instance.as_str())
                && runtime
                    .get("generation")
                    .and_then(serde_json::Value::as_str)
                    == Some(expected_generation.as_str())
                && runtime
                    .get("controlRevision")
                    .and_then(serde_json::Value::as_str)
                    == Some(expected_revision.as_str())
        });
        if !matches_expected {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_EXERCISE_UI_BINDING",
                format!(
                    "UI projection runtime identity did not match {}:{}:{}",
                    expected_instance, expected_generation, expected_revision
                ),
            ));
        }
    }
    if found {
        Ok(())
    } else {
        Err(CsharpProductRuntimeError::new(
            "CSHARP_EXERCISE_UI_BINDING",
            "admitted product update did not publish a UI projection",
        ))
    }
}

fn host_error(error: product_dev_host::ProductDevHostError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(error.code(), error.detail().to_owned())
}
fn host_runtime_error(error: product_dev_host::ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail().to_owned())
        .expect("bounded host error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static CONTENT_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn content_fixture_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "csharp-product-runtime-{label}-{}-{}",
            std::process::id(),
            CONTENT_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
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

    #[test]
    fn content_admission_quota_is_checked_without_retaining_fixture_bytes() {
        let limits = ContentAdmissionLimits {
            max_files: 2,
            max_file_bytes: 3,
            max_total_bytes: 5,
        };
        let mut quota = ContentAdmissionQuota::default();
        quota.admit(b"a", 3, limits).expect("first file at limit");
        quota.admit(b"b", 2, limits).expect("aggregate byte limit");
        assert_eq!(quota.files, 2);
        assert_eq!(quota.total_bytes, 5);

        let aggregate = quota
            .admit(b"c", 0, limits)
            .expect_err("third file exceeds count");
        assert_eq!(aggregate.code(), "CSHARP_CONTENT_LIMIT");

        let mut per_file = ContentAdmissionQuota::default();
        let error = per_file
            .admit(b"oversize", 4, limits)
            .expect_err("per-file byte limit");
        assert_eq!(error.code(), "CSHARP_CONTENT_LIMIT");

        let mut total = ContentAdmissionQuota::default();
        total.admit(b"first", 3, limits).expect("first file");
        let error = total
            .admit(b"second", 3, limits)
            .expect_err("aggregate byte limit");
        assert_eq!(error.code(), "CSHARP_CONTENT_LIMIT");
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
                product_dev_host::ProductDevRuntimeMode::Realtime,
            ),
            (
                RuntimeLifecycleConfig::Demand,
                product_dev_host::ProductDevRuntimeMode::Demand,
            ),
            (
                RuntimeLifecycleConfig::External,
                product_dev_host::ProductDevRuntimeMode::External,
            ),
        ];
        for (config, expected_mode) in cases {
            let lifecycle = RuntimeLifecycle::new(RuntimeInstanceId::new(1), config);
            assert_eq!(dev_readout(lifecycle.readout()).mode(), expected_mode);
        }
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
        let expected_faulted = ProductDevRuntimeReadout::new(
            dev_binding(faulted),
            product_dev_host::ProductDevRuntimeMode::Demand,
            ProductDevRuntimeState::Faulted,
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
        .with_fault(ProductDevRuntimeFault::OwnerReported);
        assert_eq!(dev_readout(faulted), expected_faulted);

        lifecycle.restart().expect("restart lifecycle");
        let restarted = lifecycle.readout();
        let expected_restarted = ProductDevRuntimeReadout::new(
            dev_binding(restarted),
            product_dev_host::ProductDevRuntimeMode::Demand,
            ProductDevRuntimeState::Running,
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
}
