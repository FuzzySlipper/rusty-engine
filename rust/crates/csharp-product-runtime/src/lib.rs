//! Deliberately permissive, experimental loader for one trusted NativeAOT C# product.
//!
//! This is a walking trial, not a product plugin framework or a compatibility
//! promise. The product is first-party trusted code. This adapter owns only the
//! fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its gameplay state and orchestration.

use std::{ffi::c_void, fs, path::Path, ptr};

use libloading::Library;
use product_dev_host::{
    CanonicalU64, ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeState,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};
use runtime_input::{RuntimeInputEvent, RuntimeIntentValue};
use serde::Deserialize;
use serde_json::Value;

const ABI_OK: i32 = 1;
const INSTANCE_ID: u64 = 1;
const GENERATION: u64 = 1;
const CONTROL_REVISION: u64 = 1;

/// One file made broadly available to trusted product code at creation time.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentFile {
    pub path: *const u8,
    pub path_len: usize,
    pub bytes: *const u8,
    pub bytes_len: usize,
}

/// Borrowed input supplied for the duration of one direct C# turn call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputEvent {
    pub kind: u32,
    pub edge: u32,
    pub sequence: u64,
    pub x: f32,
    pub y: f32,
    pub label: *const u8,
    pub label_len: usize,
}

/// Explicit turn timing supplied by the host; time never masquerades as input.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeTurnArgs {
    /// 1 realtime (nanoseconds), 2 demand (step), 3 external (step).
    pub kind: u32,
    pub reserved: u32,
    pub observed_time_or_step: u64,
    pub events: *const NativeInputEvent,
    pub event_count: usize,
}

/// Product-owned bytes. Rust copies them immediately and calls the matching
/// `rusty_product_free_output` exactly once before parsing the copy.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeOutputBuffer {
    pub data: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NativeCreateArgs {
    content: *const NativeContentFile,
    content_len: usize,
}

type Create =
    unsafe extern "C" fn(*const NativeCreateArgs, *mut *mut c_void, *mut NativeOutputBuffer) -> i32;
type Action = unsafe extern "C" fn(*mut c_void, *mut NativeOutputBuffer) -> i32;
type Turn =
    unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs, *mut NativeOutputBuffer) -> i32;
type Destroy = unsafe extern "C" fn(*mut c_void);
type FreeOutput = unsafe extern "C" fn(NativeOutputBuffer);

struct NativeProductApi {
    // NativeAOT initializes process-wide managed runtime support. It does not
    // provide a safe shared-library unload contract, so a successfully created
    // product keeps its library mapped until process exit after destroy.
    library: Option<Library>,
    create: Create,
    start: Action,
    turn: Turn,
    pause: Action,
    resume: Action,
    shutdown: Action,
    destroy: Destroy,
    free_output: FreeOutput,
}

impl NativeProductApi {
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
        Ok(Self {
            // Resolve every export before invoking product code, so a malformed
            // library fails as an actionable load error rather than mid-session.
            create: unsafe { symbol(&library, b"rusty_product_create\0") }?,
            start: unsafe { symbol(&library, b"rusty_product_start\0") }?,
            turn: unsafe { symbol(&library, b"rusty_product_turn\0") }?,
            pause: unsafe { symbol(&library, b"rusty_product_pause\0") }?,
            resume: unsafe { symbol(&library, b"rusty_product_resume\0") }?,
            shutdown: unsafe { symbol(&library, b"rusty_product_shutdown\0") }?,
            destroy: unsafe { symbol(&library, b"rusty_product_destroy\0") }?,
            free_output: unsafe { symbol(&library, b"rusty_product_free_output\0") }?,
            library: Some(library),
        })
    }
}

/// A loaded trusted C# product adapted to the existing local browser host.
pub struct CsharpProductRuntime {
    api: NativeProductApi,
    handle: *mut c_void,
    binding: ProductDevRuntimeBinding,
    state: ProductDevRuntimeState,
    turns: u64,
    pending_inputs: Vec<NativeInputOwned>,
    shutdown_called: bool,
}

// The development host serializes every call with one mutex. The native handle
// has no ambient access from Rust and is destroyed before the process-lifetime
// NativeAOT library mapping is retained for process exit.
unsafe impl Send for CsharpProductRuntime {}

impl CsharpProductRuntime {
    /// Loads one C# library and creates its authoritative product state.
    pub fn load(
        library_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let api = NativeProductApi::load(library_path.as_ref())?;
        let content = collect_content(content_root.as_ref())?;
        let native_content: Vec<NativeContentFile> = content
            .iter()
            .map(|file| NativeContentFile {
                path: file.path.as_ptr(),
                path_len: file.path.len(),
                bytes: file.bytes.as_ptr(),
                bytes_len: file.bytes.len(),
            })
            .collect();
        let args = NativeCreateArgs {
            content: native_content.as_ptr(),
            content_len: native_content.len(),
        };
        let mut handle = ptr::null_mut();
        let output = call_create(&api, &args, &mut handle)?;
        if handle.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_CREATE_HANDLE",
                "rusty_product_create succeeded but returned a null product handle",
            ));
        }
        // The create output is intentionally accepted as a normal first
        // projection, but this walking host has no consumer until `start`.
        decode_output(output)?;
        Ok(Self {
            api,
            handle,
            binding: binding(),
            state: ProductDevRuntimeState::Created,
            turns: 0,
            pending_inputs: Vec::new(),
            shutdown_called: false,
        })
    }

    /// Calls two direct stateful product turns and verifies their published UI
    /// values prove C# state survived the first call.
    pub fn exercise_turns(&mut self) -> Result<(), CsharpProductRuntimeError> {
        self.action(
            self.api.start,
            ProductDevOperationKind::Start,
            ProductDevRuntimeState::Running,
        )?;
        self.pending_inputs
            .push(input_owned(1, 1, 1, 0.0, 0.0, "KeyW".to_owned()));
        let first = self.turn(2, 1)?;
        let second = self.turn(2, 2)?;
        if first.turns >= second.turns || second.turns < 2 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_STATEFUL_TURNS",
                "C# turn outputs did not report increasing persistent state",
            ));
        }
        if first.input_events != 1 || second.input_events != 0 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_INPUT_TURN",
                "a queued key input was not delivered exactly once on the next C# turn",
            ));
        }
        if second.frees < 3 || second.duplicate_frees != 0 {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_OUTPUT_RELEASE",
                "C# fixture did not observe exactly one release for each preceding product output",
            ));
        }
        self.action(
            self.api.pause,
            ProductDevOperationKind::Pause,
            ProductDevRuntimeState::Paused,
        )?;
        self.action(
            self.api.resume,
            ProductDevOperationKind::Resume,
            ProductDevRuntimeState::Running,
        )?;
        Ok(())
    }

    fn turn(
        &mut self,
        kind: u32,
        observed_time_or_step: u64,
    ) -> Result<DecodedOutput, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        let output = call_turn(
            &self.api,
            self.handle,
            NativeTurnArgs {
                kind,
                reserved: 0,
                observed_time_or_step,
                events: events.as_ptr(),
                event_count: events.len(),
            },
        )?;
        // The C# call has accepted the batch. Decode errors must not replay
        // already-applied product input on a later timing turn.
        self.pending_inputs.clear();
        let decoded = decode_output(output)?;
        self.turns = self.turns.checked_add(1).ok_or_else(|| {
            CsharpProductRuntimeError::new("CSHARP_TURN_COUNTER", "turn counter overflowed")
        })?;
        Ok(decoded)
    }

    fn action(
        &mut self,
        action: Action,
        operation: ProductDevOperationKind,
        state: ProductDevRuntimeState,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        let output = call_action(&self.api, action, self.handle, operation)?;
        let decoded = decode_output(output)?;
        self.state = state;
        Ok(decoded.outputs)
    }

    fn receipt(
        &self,
        operation: ProductDevOperationKind,
        outputs: Vec<ProductDevRuntimeOutput>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let readout = self.readout();
        let result = ProductDevOperationResult::accepted(operation, self.binding, readout)
            .map_err(host_error)?;
        ProductDevRuntimeReceipt::new(result, outputs).map_err(host_runtime_error)
    }

    fn readout(&self) -> ProductDevRuntimeReadout {
        ProductDevRuntimeReadout::new(
            self.binding,
            product_dev_host::ProductDevRuntimeMode::Realtime,
            self.state,
        )
        .with_counters(self.turns, self.turns, 0, 0)
    }

    fn runtime_error(&self, error: CsharpProductRuntimeError) -> ProductDevRuntimeError {
        ProductDevRuntimeError::new(error.code, error.detail)
            .expect("fixed bounded NativeAOT error")
    }
}

impl ProductDevRuntime for CsharpProductRuntime {
    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        match operation {
            ProductDevLifecycleOperation::Start => {
                let outputs = self
                    .action(
                        self.api.start,
                        ProductDevOperationKind::Start,
                        ProductDevRuntimeState::Running,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Start, outputs)
            }
            ProductDevLifecycleOperation::Pause => {
                let outputs = self
                    .action(
                        self.api.pause,
                        ProductDevOperationKind::Pause,
                        ProductDevRuntimeState::Paused,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Pause, outputs)
            }
            ProductDevLifecycleOperation::Resume => {
                let outputs = self
                    .action(
                        self.api.resume,
                        ProductDevOperationKind::Resume,
                        ProductDevRuntimeState::Running,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                self.receipt(ProductDevOperationKind::Resume, outputs)
            }
            ProductDevLifecycleOperation::Shutdown => {
                let outputs = self
                    .action(
                        self.api.shutdown,
                        ProductDevOperationKind::Shutdown,
                        ProductDevRuntimeState::Shutdown,
                    )
                    .map_err(|error| self.runtime_error(error))?;
                let receipt = self.receipt(ProductDevOperationKind::Shutdown, outputs);
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

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError> {
        self.pending_inputs.extend(native_events(batch.events()));
        let result =
            ProductDevInputResult::accepted(batch.events().len(), self.binding, self.readout())
                .map_err(host_error)?;
        ProductDevRuntimeReceipt::new(result, Vec::new()).map_err(host_runtime_error)
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let decoded = self
            .turn(1, observed_time_ns.get())
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdvanceRealtime, decoded.outputs)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let decoded = self
            .turn(2, self.turns.saturating_add(1))
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitDemandStep, decoded.outputs)
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let decoded = self
            .turn(3, step.get())
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitExternalStep, decoded.outputs)
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
            let _ = unsafe { (self.api.shutdown)(self.handle, ptr::null_mut()) };
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

fn binding() -> ProductDevRuntimeBinding {
    ProductDevRuntimeBinding {
        instance_id: CanonicalU64::new(INSTANCE_ID),
        generation: CanonicalU64::new(GENERATION),
        control_revision: CanonicalU64::new(CONTROL_REVISION),
    }
}

fn call_create(
    api: &NativeProductApi,
    args: &NativeCreateArgs,
    handle: &mut *mut c_void,
) -> Result<Vec<u8>, CsharpProductRuntimeError> {
    let mut output = NativeOutputBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    // SAFETY: fixed ABI pointers are valid for the duration of this call.
    let status = unsafe { (api.create)(args, handle, &mut output) };
    copied_output(api, status, output, "create")
}

fn call_action(
    api: &NativeProductApi,
    action: Action,
    handle: *mut c_void,
    operation: ProductDevOperationKind,
) -> Result<Vec<u8>, CsharpProductRuntimeError> {
    let mut output = NativeOutputBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    // SAFETY: `handle` is retained by the runtime and output is stack-owned.
    let status = unsafe { action(handle, &mut output) };
    copied_output(api, status, output, operation_name(operation))
}

fn call_turn(
    api: &NativeProductApi,
    handle: *mut c_void,
    args: NativeTurnArgs,
) -> Result<Vec<u8>, CsharpProductRuntimeError> {
    let mut output = NativeOutputBuffer {
        data: ptr::null_mut(),
        len: 0,
    };
    // SAFETY: event label pointers borrow local strings that remain alive for
    // the call; the C# product is required to copy anything it retains.
    let status = unsafe { (api.turn)(handle, &args, &mut output) };
    copied_output(api, status, output, "turn")
}

fn copied_output(
    api: &NativeProductApi,
    status: i32,
    output: NativeOutputBuffer,
    operation: &str,
) -> Result<Vec<u8>, CsharpProductRuntimeError> {
    if status != ABI_OK {
        if !output.data.is_null() {
            // SAFETY: an error-status buffer is still product-owned output and
            // must be released exactly once even though Rust does not inspect it.
            unsafe { (api.free_output)(output) };
        }
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_PRODUCT_CALL",
            format!("C# product {operation} returned status {status}"),
        ));
    }
    if output.len > 0 && output.data.is_null() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_OUTPUT_POINTER",
            format!("C# product {operation} returned a null output pointer with non-zero length"),
        ));
    }
    // SAFETY: a non-null product buffer is valid for its reported length until
    // `free_output`. Copying happens before release and release happens once.
    let copied = if output.len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(output.data, output.len).to_vec() }
    };
    if !output.data.is_null() {
        // SAFETY: this is the one owning release for the returned buffer.
        unsafe { (api.free_output)(output) };
    }
    Ok(copied)
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
    bytes: Vec<u8>,
}

fn collect_content(root: &Path) -> Result<Vec<ContentFile>, CsharpProductRuntimeError> {
    if !root.is_dir() {
        return Err(CsharpProductRuntimeError::new(
            "CSHARP_CONTENT_ROOT",
            format!("content directory does not exist: {}", root.display()),
        ));
    }
    let mut files = Vec::new();
    collect_content_inner(root, root, &mut files)?;
    Ok(files)
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
                bytes: fs::read(&entry.path()).map_err(|error| {
                    CsharpProductRuntimeError::new("CSHARP_CONTENT_READ", error.to_string())
                })?,
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
}

struct NativeInputOwned {
    kind: u32,
    edge: u32,
    sequence: u64,
    x: f32,
    y: f32,
    label: Vec<u8>,
}

impl NativeInputOwned {
    fn as_native(&self) -> NativeInputEvent {
        NativeInputEvent {
            kind: self.kind,
            edge: self.edge,
            sequence: self.sequence,
            x: self.x,
            y: self.y,
            label: self.label.as_ptr(),
            label_len: self.label.len(),
        }
    }
}

fn native_events(events: &[RuntimeInputEvent]) -> Vec<NativeInputOwned> {
    events.iter().map(native_event).collect()
}

fn native_event(event: &RuntimeInputEvent) -> NativeInputOwned {
    match event {
        RuntimeInputEvent::Physical(physical) => {
            let (kind, edge, x, y, label) = match physical.fact() {
                runtime_input::RuntimeInputFact::Key { code, edge } => {
                    (1, edge_value(*edge), 0.0, 0.0, format!("{code:?}"))
                }
                runtime_input::RuntimeInputFact::PointerButton { button, edge } => {
                    (2, edge_value(*edge), 0.0, 0.0, format!("{button:?}"))
                }
                runtime_input::RuntimeInputFact::PointerDelta { x, y } => {
                    (3, 0, x.value(), y.value(), String::new())
                }
                runtime_input::RuntimeInputFact::Wheel { x, y } => {
                    (4, 0, x.value(), y.value(), String::new())
                }
                runtime_input::RuntimeInputFact::ControllerButton { button, edge } => {
                    (5, edge_value(*edge), 0.0, 0.0, format!("{button:?}"))
                }
                runtime_input::RuntimeInputFact::ControllerAxis { axis, value } => {
                    (6, 0, value.value(), 0.0, format!("{axis:?}"))
                }
                runtime_input::RuntimeInputFact::Clear { reason } => {
                    (7, 0, 0.0, 0.0, format!("{reason:?}"))
                }
            };
            input_owned(kind, edge, physical.sequence(), x, y, label)
        }
        RuntimeInputEvent::DirectIntent(intent) => {
            let (kind, x, y) = match intent.value() {
                RuntimeIntentValue::Digital { active } => (8, if active { 1.0 } else { 0.0 }, 0.0),
                RuntimeIntentValue::Axis { value } => (9, value.value(), 0.0),
                RuntimeIntentValue::ProductPayload { .. } => (10, 0.0, 0.0),
            };
            input_owned(kind, 0, intent.sequence(), x, y, intent.intent().to_owned())
        }
    }
}

fn input_owned(
    kind: u32,
    edge: u32,
    sequence: u64,
    x: f32,
    y: f32,
    label: String,
) -> NativeInputOwned {
    let label = label.into_bytes();
    NativeInputOwned {
        kind,
        edge,
        sequence,
        x,
        y,
        label,
    }
}

fn edge_value(edge: runtime_input::PhysicalEdge) -> u32 {
    match edge {
        runtime_input::PhysicalEdge::Pressed => 1,
        runtime_input::PhysicalEdge::Released => 2,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductOutputWire {
    #[serde(default)]
    ui: Option<Value>,
    #[serde(default)]
    frame: Option<Value>,
    #[serde(default)]
    turns: Option<u64>,
    #[serde(default)]
    frees: Option<u64>,
    #[serde(default, rename = "duplicateFrees")]
    duplicate_frees: Option<u64>,
    #[serde(default, rename = "inputEvents")]
    input_events: Option<u64>,
}

struct DecodedOutput {
    outputs: Vec<ProductDevRuntimeOutput>,
    turns: u64,
    frees: u64,
    duplicate_frees: u64,
    input_events: u64,
}

fn decode_output(bytes: Vec<u8>) -> Result<DecodedOutput, CsharpProductRuntimeError> {
    if bytes.is_empty() {
        return Ok(DecodedOutput {
            outputs: Vec::new(),
            turns: 0,
            frees: 0,
            duplicate_frees: 0,
            input_events: 0,
        });
    }
    let wire: ProductOutputWire = serde_json::from_slice(&bytes)
        .map_err(|error| CsharpProductRuntimeError::new("CSHARP_OUTPUT_JSON", error.to_string()))?;
    let mut outputs = Vec::new();
    if let Some(ui) = wire.ui {
        let encoded = serde_json::to_vec(&ui).map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_UI_ENCODE", error.to_string())
        })?;
        let envelope =
            runtime_ui::RuntimeUiProjectionEnvelope::decode_json(&encoded).map_err(|_| {
                CsharpProductRuntimeError::new(
                    "CSHARP_UI_DECODE",
                    "C# product returned an invalid runtime-ui projection",
                )
            })?;
        outputs.push(ProductDevRuntimeOutput::ui_projection(&envelope).map_err(host_error)?);
    }
    if let Some(frame) = wire.frame {
        let frame =
            render_model::RenderFrameDiff::decode_json(&frame.to_string()).map_err(|_| {
                CsharpProductRuntimeError::new(
                    "CSHARP_RENDER_DECODE",
                    "C# product returned an invalid retained render frame",
                )
            })?;
        outputs.push(ProductDevRuntimeOutput::frame(&frame).map_err(host_error)?);
    }
    Ok(DecodedOutput {
        outputs,
        turns: wire.turns.unwrap_or(0),
        frees: wire.frees.unwrap_or(0),
        duplicate_frees: wire.duplicate_frees.unwrap_or(0),
        input_events: wire.input_events.unwrap_or(0),
    })
}

fn host_error(error: product_dev_host::ProductDevHostError) -> CsharpProductRuntimeError {
    CsharpProductRuntimeError::new(error.code(), error.detail().to_owned())
}
fn host_runtime_error(error: product_dev_host::ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail().to_owned())
        .expect("bounded host error")
}

#[derive(Debug)]
pub struct CsharpProductRuntimeError {
    code: &'static str,
    detail: String,
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
impl From<CsharpProductRuntimeError> for ProductDevRuntimeError {
    fn from(error: CsharpProductRuntimeError) -> Self {
        ProductDevRuntimeError::new(error.code, error.detail)
            .expect("fixed bounded NativeAOT error")
    }
}
