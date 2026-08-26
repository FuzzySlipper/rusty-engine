//! Deliberately permissive, experimental loader for one trusted NativeAOT C# product.
//!
//! This is a walking trial, not a product plugin framework or a compatibility
//! promise. The product is first-party trusted code. This adapter owns only the
//! fixed C ABI, copying borrowed/owned buffers, and deterministic library
//! lifetime; the C# product owns its gameplay state and orchestration.

mod native_api;

pub use native_api::*;

use std::{ffi::c_void, fs, path::Path, ptr};

use libloading::Library;
use product_dev_host::{
    CanonicalU64, ProductDevInputBatch, ProductDevInputResult, ProductDevLifecycleOperation,
    ProductDevOperationKind, ProductDevOperationResult, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevRuntimeReadout, ProductDevRuntimeReceipt, ProductDevRuntimeState,
    ProductDevTimelineCompletion, ProductDevTimelineCompletionResult,
};
use render_model::Transform;
use render_projection::{
    RuntimeAppearanceCatalog, RuntimeAppearanceFact, RuntimeAppearanceProjector,
};
use runtime_input::{RuntimeInputEvent, RuntimeIntentValue};

const ABI_OK: i32 = 1;
const INSTANCE_ID: u64 = 1;
const GENERATION: u64 = 1;
const CONTROL_REVISION: u64 = 1;

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
    binding: ProductDevRuntimeBinding,
    state: ProductDevRuntimeState,
    turns: u64,
    pending_inputs: Vec<NativeInputOwned>,
    visual_bridge: Box<RuntimeAppearanceBridge>,
    initial_frame: Option<render_model::RenderFrameDiff>,
    last_visual_frame: Option<render_model::RenderFrameDiff>,
    shutdown_called: bool,
}

// The development host serializes every call with one mutex. The native handle
// has no ambient access from Rust and is destroyed before the process-lifetime
// NativeAOT library mapping is retained for process exit.
unsafe impl Send for CsharpProductRuntime {}

struct StagedVisualProjection {
    projector: RuntimeAppearanceProjector,
    frame: render_model::RenderFrameDiff,
}

/// Callback state remains Engine-owned for the complete NativeAOT runtime lifetime.
/// A C# call stages against a clone and only publishes its retained changes once the
/// matching native call has returned successfully.
struct RuntimeAppearanceBridge {
    projector: RuntimeAppearanceProjector,
    staged: Option<StagedVisualProjection>,
    callback_error: Option<CsharpProductRuntimeError>,
}

impl RuntimeAppearanceBridge {
    fn new(catalog: RuntimeAppearanceCatalog) -> Self {
        Self {
            projector: RuntimeAppearanceProjector::new(catalog),
            staged: None,
            callback_error: None,
        }
    }

    fn function_table(&mut self) -> NativeEngineApi {
        NativeEngineApi {
            context: (self as *mut Self).cast(),
            publish_visual_snapshot,
        }
    }

    fn begin_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    fn take_staged_call(
        &mut self,
    ) -> Result<Option<StagedVisualProjection>, CsharpProductRuntimeError> {
        if let Some(error) = self.callback_error.take() {
            self.staged = None;
            return Err(error);
        }
        Ok(self.staged.take())
    }

    fn commit(&mut self, staged: Option<StagedVisualProjection>) {
        let Some(staged) = staged else {
            return;
        };
        self.projector = staged.projector;
    }

    unsafe fn stage_snapshot(
        &mut self,
        facts: *const NativeVisualFact,
        fact_count: usize,
    ) -> Result<(), CsharpProductRuntimeError> {
        if fact_count > 0 && facts.is_null() {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_FACT_POINTER",
                "C# visual snapshot had a null fact pointer with non-zero count",
            ));
        }
        // SAFETY: the fixed callback contract keeps the array alive for this
        // synchronous call, and the decoded facts copy all borrowed identity bytes.
        let facts = if fact_count == 0 {
            &[]
        } else {
            // SAFETY: non-zero fact counts were checked for a non-null pointer.
            unsafe { std::slice::from_raw_parts(facts, fact_count) }
        };
        let mut decoded = Vec::with_capacity(facts.len());
        for fact in facts {
            if fact.appearance_len > 0 && fact.appearance.is_null() {
                return Err(CsharpProductRuntimeError::new(
                    "CSHARP_VISUAL_APPEARANCE_POINTER",
                    "C# visual fact had a null appearance pointer with non-zero length",
                ));
            }
            // Zero length is handled without constructing a raw slice; non-zero
            // pointers were checked above. UTF-8 identity bytes are copied before return.
            let appearance = if fact.appearance_len == 0 {
                &[]
            } else {
                // SAFETY: non-zero appearance lengths were checked for a non-null pointer.
                unsafe { std::slice::from_raw_parts(fact.appearance, fact.appearance_len) }
            };
            let appearance = std::str::from_utf8(appearance).map_err(|_| {
                CsharpProductRuntimeError::new(
                    "CSHARP_VISUAL_APPEARANCE_UTF8",
                    "C# visual fact appearance identity was not UTF-8",
                )
            })?;
            decoded.push(RuntimeAppearanceFact {
                object_id: fact.object_id,
                appearance: appearance.to_owned(),
                transform: Transform {
                    translation: fact.translation,
                    rotation: fact.rotation,
                    scale: fact.scale,
                },
                visible: fact.visible != 0,
            });
        }
        let mut candidate = self.projector.clone();
        let projected = candidate.project(&decoded).map_err(|error| {
            CsharpProductRuntimeError::new("CSHARP_VISUAL_PROJECT", format!("{error:?}"))
        })?;
        self.staged = Some(StagedVisualProjection {
            projector: candidate,
            frame: projected.frame,
        });
        Ok(())
    }
}

unsafe extern "C" fn publish_visual_snapshot(
    context: *mut c_void,
    facts: *const NativeVisualFact,
    fact_count: usize,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    // SAFETY: `context` is a stable pointer to the Box retained by
    // `CsharpProductRuntime`, and calls are serialized by the development host.
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    // SAFETY: all raw callback pointers are validated and copied by this helper.
    match unsafe { bridge.stage_snapshot(facts, fact_count) } {
        Ok(()) => 1,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

impl CsharpProductRuntime {
    /// Loads one C# library and creates its authoritative product state.
    pub fn load(
        library_path: impl AsRef<Path>,
        content_root: impl AsRef<Path>,
    ) -> Result<Self, CsharpProductRuntimeError> {
        let api = LoadedProductApi::load(library_path.as_ref())?;
        let content = collect_content(content_root.as_ref())?;
        let catalog = load_runtime_appearance_catalog(content_root.as_ref())?;
        let mut visual_bridge = Box::new(RuntimeAppearanceBridge::new(catalog));
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
            engine: visual_bridge.function_table(),
        };
        let mut handle = ptr::null_mut();
        visual_bridge.begin_call();
        match call_create(&api, &args, &mut handle) {
            Ok(()) => {}
            Err(error) => {
                visual_bridge.discard_call();
                if !handle.is_null() {
                    // SAFETY: a failing create may still have returned an owned
                    // handle; releasing it is part of the fixed ownership ABI.
                    unsafe { (api.destroy)(handle) };
                }
                return Err(error);
            }
        }
        let staged = match visual_bridge.take_staged_call() {
            Ok(staged) => staged,
            Err(error) => {
                if !handle.is_null() {
                    // SAFETY: successful create produced this owned product handle,
                    // but its staged callback output was not accepted by Rust.
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
        let initial_frame = staged.as_ref().map(|staged| staged.frame.clone());
        visual_bridge.commit(staged);
        Ok(Self {
            api,
            handle,
            binding: binding(),
            state: ProductDevRuntimeState::Created,
            turns: 0,
            pending_inputs: Vec::new(),
            visual_bridge,
            initial_frame,
            last_visual_frame: None,
            shutdown_called: false,
        })
    }

    /// Calls two direct stateful product turns and verifies their Engine-owned
    /// retained projection proves C# state survived the first call.
    pub fn exercise_turns(&mut self) -> Result<(), CsharpProductRuntimeError> {
        self.action(
            self.api.start,
            ProductDevOperationKind::Start,
            ProductDevRuntimeState::Running,
        )?;
        let start_frame = self.last_visual_frame.as_ref().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_START_FRAME",
                "C# start did not publish an Engine-projected visual snapshot",
            )
        })?;
        if !start_frame.ops.iter().any(|operation| {
            matches!(
                operation,
                render_model::RenderDiff::DefineStaticMesh { asset }
                    if asset.asset == "mesh/nativeaot-trial"
            )
        }) {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_START_DEFINE",
                "C# start did not define the admitted Engine static mesh",
            ));
        }
        let handle = start_frame
            .ops
            .iter()
            .find_map(|operation| match operation {
                render_model::RenderDiff::CreateStaticMeshInstance { handle, .. } => Some(*handle),
                _ => None,
            })
            .ok_or_else(|| {
                CsharpProductRuntimeError::new(
                    "CSHARP_VISUAL_START_CREATE",
                    "C# start did not create an Engine static-mesh instance",
                )
            })?;
        self.pending_inputs
            .push(input_owned(1, 1, 1, 0.0, 0.0, "KeyW".to_owned()));
        self.turn(2, 1)?;
        let first_frame = self.last_visual_frame.as_ref().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_TURN_UPDATE",
                "first C# turn did not publish an Engine-projected visual snapshot",
            )
        })?;
        if !first_frame.ops.iter().any(|operation| match operation {
            render_model::RenderDiff::Update {
                handle: updated,
                transform: Some(transform),
                ..
            } => *updated == handle && transform.translation == [1.0, 0.0, 0.0],
            _ => false,
        }) {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_HANDLE_UPDATE",
                "queued KeyW input did not move the same retained Engine handle",
            ));
        }
        self.turn(2, 2)?;
        let second_frame = self.last_visual_frame.as_ref().ok_or_else(|| {
            CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_TURN_DESTROY",
                "second C# turn did not publish an Engine-projected visual snapshot",
            )
        })?;
        if !second_frame.ops.iter().any(|operation| {
            matches!(
                operation,
                render_model::RenderDiff::Destroy { handle: destroyed } if *destroyed == handle
            )
        }) {
            return Err(CsharpProductRuntimeError::new(
                "CSHARP_VISUAL_HANDLE_DESTROY",
                "second C# turn did not destroy the retained Engine handle",
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
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        let events: Vec<NativeInputEvent> = self
            .pending_inputs
            .iter()
            .map(NativeInputOwned::as_native)
            .collect();
        self.visual_bridge.begin_call();
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
                self.visual_bridge.discard_call();
                return Err(error);
            }
        }
        let staged = self.visual_bridge.take_staged_call()?;
        // The C# call has accepted the batch. Do not replay already-applied
        // product input on a later timing turn.
        self.pending_inputs.clear();
        let mut outputs = Vec::new();
        append_frame(
            &mut outputs,
            staged.as_ref().map(|staged| staged.frame.clone()),
        )?;
        let turns = self.turns.checked_add(1).ok_or_else(|| {
            CsharpProductRuntimeError::new("CSHARP_TURN_COUNTER", "turn counter overflowed")
        })?;
        self.last_visual_frame = staged.as_ref().map(|staged| staged.frame.clone());
        self.visual_bridge.commit(staged);
        self.turns = turns;
        Ok(outputs)
    }

    fn action(
        &mut self,
        action: NativeProductAction,
        operation: ProductDevOperationKind,
        state: ProductDevRuntimeState,
    ) -> Result<Vec<ProductDevRuntimeOutput>, CsharpProductRuntimeError> {
        self.visual_bridge.begin_call();
        match call_action(action, self.handle, operation) {
            Ok(()) => {}
            Err(error) => {
                self.visual_bridge.discard_call();
                return Err(error);
            }
        }
        let staged = self.visual_bridge.take_staged_call()?;
        let mut outputs = Vec::new();
        append_frame(&mut outputs, self.initial_frame.clone())?;
        append_frame(
            &mut outputs,
            staged.as_ref().map(|staged| staged.frame.clone()),
        )?;
        self.initial_frame = None;
        self.last_visual_frame = staged.as_ref().map(|staged| staged.frame.clone());
        self.visual_bridge.commit(staged);
        self.state = state;
        Ok(outputs)
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
        let outputs = self
            .turn(1, observed_time_ns.get())
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdvanceRealtime, outputs)
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let outputs = self
            .turn(2, self.turns.saturating_add(1))
            .map_err(|error| self.runtime_error(error))?;
        self.receipt(ProductDevOperationKind::AdmitDemandStep, outputs)
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        let outputs = self
            .turn(3, step.get())
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

fn binding() -> ProductDevRuntimeBinding {
    ProductDevRuntimeBinding {
        instance_id: CanonicalU64::new(INSTANCE_ID),
        generation: CanonicalU64::new(GENERATION),
        control_revision: CanonicalU64::new(CONTROL_REVISION),
    }
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

fn load_runtime_appearance_catalog(
    root: &Path,
) -> Result<RuntimeAppearanceCatalog, CsharpProductRuntimeError> {
    let path = root.join("runtime-appearances.json");
    let bytes = fs::read(&path).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_RUNTIME_APPEARANCES_READ",
            format!("{}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CsharpProductRuntimeError::new(
            "CSHARP_RUNTIME_APPEARANCES_DECODE",
            format!("{}: {error}", path.display()),
        )
    })
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
                bytes: fs::read(entry.path()).map_err(|error| {
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

fn append_frame(
    outputs: &mut Vec<ProductDevRuntimeOutput>,
    frame: Option<render_model::RenderFrameDiff>,
) -> Result<(), CsharpProductRuntimeError> {
    if let Some(frame) = frame {
        outputs.push(ProductDevRuntimeOutput::frame(&frame).map_err(host_error)?);
    }
    Ok(())
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
