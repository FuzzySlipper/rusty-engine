//! Deliberately permissive, experimental loader for one trusted NativeAOT C# product.
//!
//! This is a walking trial, not a product plugin framework or a compatibility
//! promise. The product is first-party trusted code. This adapter owns only the
//! fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its gameplay state and orchestration.

pub use csharp_engine_abi::*;

use std::{ffi::c_void, fs, path::Path, ptr, sync::Arc};

use csharp_engine_services::{
    CsharpAppearanceCatalog, CsharpEngineServicesError, CsharpRenderResource,
    CsharpRenderResourceKind, EngineServiceSet,
};
use libloading::Library;
use product_dev_host::{
    CanonicalU64, ProductDevControlOperation, ProductDevInputBatch, ProductDevInputResult,
    ProductDevLifecycleOperation, ProductDevOperationKind, ProductDevOperationResult,
    ProductDevRendererResource, ProductDevRuntime, ProductDevRuntimeBinding,
    ProductDevRuntimeError, ProductDevRuntimeOutput, ProductDevRuntimeReadout,
    ProductDevRuntimeReceipt, ProductDevRuntimeState, ProductDevTimelineCompletion,
    ProductDevTimelineCompletionResult,
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
        }
    }

    /// Adds typed standard-runtime physical mappings to create-time host
    /// configuration. The product receives a copied descriptor; it does not
    /// own or mutate the runtime lane's mapping evaluation.
    pub fn with_physical_mappings(mut self, mappings: Vec<RuntimeInputMapping>) -> Self {
        self.physical_mappings = mappings;
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
        let mut services = Box::new(EngineServiceSet::new(appearance_catalog, content_resources));
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
        services.begin_call();
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
        kind: NativeProductTurnKind,
        observed_time_or_step: u64,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        self.services.begin_call();
        match call_turn(
            &self.api,
            self.handle,
            NativeTurnArgs {
                kind,
                reserved: 0,
                observed_time_or_step,
                events: events.as_ptr(),
                event_count: events.len(),
            },
        ) {
            Ok(()) => {}
            Err(error) => {
                self.services.discard_call();
                return Err(error);
            }
        }
        let staged = match self.services.take_call() {
            Ok(staged) => staged,
            Err(error) => {
                self.services.discard_call();
                return Err(error.into());
            }
        };
        // The C# call has accepted the batch. Do not replay already-applied
        // product input on a later timing turn.
        self.pending_inputs.clear();
        let outputs = service_outputs(self.services.outputs(&staged))?;
        self.services.commit_call(staged);
        Ok(outputs)
    }

    fn turn_admitted(
        &mut self,
        kind: NativeProductTurnKind,
        observed_time_or_step: u64,
        admission: runtime_lifecycle::SimulationAdmission,
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
        self.turn(kind, observed_time_or_step)
    }

    fn action(
        &mut self,
        action: NativeProductAction,
        operation: ProductDevOperationKind,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        self.services.begin_call();
        match call_action(action, self.handle, operation) {
            Ok(()) => {}
            Err(error) => {
                self.services.discard_call();
                return Err(error);
            }
        }
        let staged = match self.services.take_call() {
            Ok(staged) => staged,
            Err(error) => {
                self.services.discard_call();
                return Err(error.into());
            }
        };
        let mut outputs = if matches!(operation, ProductDevOperationKind::Start) {
            std::mem::take(&mut self.initial_outputs)
        } else {
            Vec::new()
        };
        outputs.extend(service_outputs(self.services.outputs(&staged))?);
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
        self.action(self.api.start, ProductDevOperationKind::Start)?;
        self.lifecycle.start().map_err(lifecycle_error)?;
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
                    .action(self.api.start, ProductDevOperationKind::Start)
                    .map_err(|error| self.runtime_error(error))?;
                self.lifecycle.start().map_err(lifecycle_runtime_error)?;
                self.rebind_input(InputClearReason::Restart)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Start,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Pause => {
                let outputs = self
                    .action(self.api.pause, ProductDevOperationKind::Pause)
                    .map_err(|error| self.runtime_error(error))?;
                self.lifecycle.pause().map_err(lifecycle_runtime_error)?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Pause,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Resume => {
                let outputs = self
                    .action(self.api.resume, ProductDevOperationKind::Resume)
                    .map_err(|error| self.runtime_error(error))?;
                self.lifecycle.resume().map_err(lifecycle_runtime_error)?;
                self.rebind_input(InputClearReason::ControlRevisionChange)
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(
                    ProductDevOperationKind::Resume,
                    self.tag_complete_baseline(outputs),
                )
            }
            ProductDevLifecycleOperation::Shutdown => {
                let outputs = self
                    .action(self.api.shutdown, ProductDevOperationKind::Shutdown)
                    .map_err(|error| self.runtime_error(error))?;
                self.lifecycle.shutdown().map_err(lifecycle_runtime_error)?;
                self.input_lane.dispose();
                self.pending_inputs.clear();
                let receipt = self.receipt(
                    ProductDevOperationKind::Shutdown,
                    self.tag_complete_baseline(outputs),
                );
                self.shutdown_called = receipt.is_ok();
                receipt
            }
            ProductDevLifecycleOperation::Restart | ProductDevLifecycleOperation::ReportFault => {
                Err(ProductDevRuntimeError::new(
                    "CSHARP_UNSUPPORTED_LIFECYCLE",
                    "this trusted NativeAOT trial exposes only start, pause, resume, and shutdown",
                )
                .expect("fixed error"))
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
            Some(admission) => self
                .turn_admitted(REALTIME_TURN_KIND, observed_time_ns.get(), admission)
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
            .turn_admitted(DEMAND_TURN_KIND, admission.first_step().value(), admission)
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
            .turn_admitted(
                EXTERNAL_TURN_KIND,
                admission.first_step().value(),
                admission,
            )
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
    ProductDevRuntimeReadout::new(dev_binding(readout), mode, state)
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
        )
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
) -> Result<(), CsharpProductRuntimeError> {
    // SAFETY: event label pointers borrow local strings that remain alive for
    // the call; the C# product is required to copy anything it retains.
    let status = unsafe { (api.turn)(handle, &args) };
    checked_status(status, "turn")
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
        ProductDevOperationKind::Shutdown => "shutdown",
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
    files: Vec<ContentFile>,
    appearance_catalog: CsharpAppearanceCatalog,
}

impl CsharpProductContent {
    pub fn admit(root: impl AsRef<Path>) -> Result<Self, CsharpProductRuntimeError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CONTENT_ROOT",
                format!("content directory does not exist: {}", root.display()),
            ));
        }
        let mut files = Vec::new();
        collect_content_inner(root, root, &mut files)?;
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

fn collect_content_inner(
    root: &Path,
    directory: &Path,
    files: &mut Vec<ContentFile>,
) -> Result<(), CsharpProductRuntimeError> {
    for entry in fs::read_dir(directory)
        .map_err(|error| CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string()))?
    {
        let entry = entry.map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_content_inner(root, &path, files)?;
        } else if path.is_file() {
            let relative = path.strip_prefix(root).expect("walked below content root");
            let path = relative.to_string_lossy().replace('\\', "/").into_bytes();
            files.push(ContentFile {
                path,
                bytes: Arc::from(fs::read(entry.path()).map_err(|error| {
                    CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
                })?),
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
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

    #[test]
    fn content_collection_leaves_unselected_png_bytes_unvalidated() {
        let root = std::env::temp_dir().join(format!(
            "csharp-product-runtime-content-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("content root");
        fs::write(root.join("unrelated-ui.png"), b"not an RGBA PNG").expect("content file");

        let content = CsharpProductContent::admit(&root)
            .expect("collect content without admitting resources");
        fs::remove_dir_all(&root).expect("remove fixture");

        assert_eq!(content.files.len(), 1);
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
}
