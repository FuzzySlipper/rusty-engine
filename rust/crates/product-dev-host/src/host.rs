use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use runtime_input::decode_runtime_input_wire_events_json;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::{
    CanonicalU64, ProductDevBundle, ProductDevControlOperation, ProductDevHostError,
    ProductDevInputBatch, ProductDevLifecycleOperation, ProductDevOperationKind,
    ProductDevOperationResult, ProductDevRuntime, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevTimelineCompletion, MAX_CONNECTIONS, MAX_OUTPUT_AGGREGATE_BYTES,
    MAX_OUTPUT_EVENT_BYTES, MAX_OUTPUT_FRAGMENT_DATA_BYTES, MAX_OUTPUT_QUEUE_ITEMS,
    MAX_REQUEST_BODY_BYTES, MAX_REQUEST_HEADER_BYTES, MAX_SSE_SUBSCRIBERS,
    PRODUCT_DEV_RUNTIME_BASE_PATH,
};

const SOCKET_TIMEOUT: Duration = Duration::from_millis(750);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const SSE_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Configuration for the fixed development host.
#[derive(Debug, Clone)]
pub struct ProductDevHostConfig {
    /// `0` asks the operating system for a free port.
    pub port: u16,
    pub bundle: ProductDevBundle,
    bind_host: Ipv4Addr,
    live_debug_enabled: bool,
}

impl ProductDevHostConfig {
    pub fn new(port: u16, bundle: ProductDevBundle) -> Self {
        Self {
            port,
            bundle,
            bind_host: Ipv4Addr::LOCALHOST,
            live_debug_enabled: false,
        }
    }

    /// Selects an explicit trusted development-network listener. Loopback is
    /// the default; `0.0.0.0` is intended for a foreground owner such as
    /// den-serve that publishes the resulting LAN origin.
    pub fn with_bind_host(mut self, bind_host: Ipv4Addr) -> Self {
        self.bind_host = bind_host;
        self
    }

    /// Explicitly admits the trusted first-party product live-debug routes.
    /// They are absent by default so ordinary dev hosts do not expose a
    /// command endpoint accidentally.
    pub fn with_live_debug(mut self, enabled: bool) -> Self {
        self.live_debug_enabled = enabled;
        self
    }
}

/// Starts an Engine-owned local development host for one concrete runtime.
pub struct ProductDevHost;

impl ProductDevHost {
    pub fn start<R: ProductDevRuntime>(
        runtime: R,
        config: ProductDevHostConfig,
    ) -> Result<RunningProductDevHost, ProductDevHostError> {
        let listener = TcpListener::bind(SocketAddr::from((config.bind_host, config.port)))
            .map_err(|error| ProductDevHostError::io("DEV_HOST_BIND", error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| ProductDevHostError::io("DEV_HOST_LISTENER_MODE", error))?;
        let address = listener
            .local_addr()
            .map_err(|error| ProductDevHostError::io("DEV_HOST_ADDRESS", error))?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let state = Arc::new(HostState {
            bundle: config.bundle,
            runtime: Mutex::new(runtime),
            outputs: Mutex::new(OutputBus::default()),
            shutdown: Arc::clone(&shutdown),
            bind_host: config.bind_host,
            expected_port: address.port(),
            live_debug_enabled: config.live_debug_enabled,
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
        });
        let handler_threads = Arc::new(Mutex::new(Vec::new()));
        let listener_state = Arc::clone(&state);
        let listener_threads = Arc::clone(&handler_threads);
        let listener_thread = thread::Builder::new()
            .name("rusty-product-dev-host".to_owned())
            .spawn(move || accept_loop(listener, listener_state, listener_threads))
            .map_err(|error| ProductDevHostError::io("DEV_HOST_THREAD", error))?;
        Ok(RunningProductDevHost {
            address,
            shutdown,
            listener_thread: Some(listener_thread),
            handler_threads,
        })
    }
}

/// A running development host. Shutdown is explicit and joins every accepted
/// connection handler so tests and generated launchers do not leak threads.
pub struct RunningProductDevHost {
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    listener_thread: Option<JoinHandle<()>>,
    handler_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl RunningProductDevHost {
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn shutdown(mut self) -> Result<(), ProductDevHostError> {
        self.stop()
    }

    fn stop(&mut self) -> Result<(), ProductDevHostError> {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        // Wake a nonblocking accept loop promptly. The connection is accepted
        // and observes the same shutdown flag before it parses a request.
        let _ = TcpStream::connect_timeout(&self.address, SOCKET_TIMEOUT);
        if let Some(thread) = self.listener_thread.take() {
            thread.join().map_err(|_| {
                ProductDevHostError::new("DEV_HOST_THREAD_JOIN", "listener thread panicked")
            })?;
        }
        let handlers = std::mem::take(&mut *self.handler_threads.lock().map_err(|_| {
            ProductDevHostError::new("DEV_HOST_THREAD_JOIN", "handler thread ledger poisoned")
        })?);
        for handler in handlers {
            handler.join().map_err(|_| {
                ProductDevHostError::new("DEV_HOST_THREAD_JOIN", "connection handler panicked")
            })?;
        }
        Ok(())
    }
}

impl Drop for RunningProductDevHost {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct HostState<R> {
    bundle: ProductDevBundle,
    runtime: Mutex<R>,
    outputs: Mutex<OutputBus>,
    shutdown: Arc<AtomicBool>,
    bind_host: Ipv4Addr,
    expected_port: u16,
    live_debug_enabled: bool,
    connections: AtomicUsize,
    subscribers: AtomicUsize,
}

fn accept_loop<R: ProductDevRuntime>(
    listener: TcpListener,
    state: Arc<HostState<R>>,
    handler_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
) {
    while !state.shutdown.load(Ordering::Acquire) {
        reap_finished_handlers(&handler_threads);
        match listener.accept() {
            Ok((stream, _)) => {
                if !try_acquire(&state.connections, MAX_CONNECTIONS) {
                    let mut stream = stream;
                    let _ = write_response(
                        &mut stream,
                        HttpResponse::error(
                            503,
                            "DEV_HOST_CONNECTION_BOUNDS",
                            "connection limit reached",
                        ),
                    );
                    continue;
                }
                let connection_state = Arc::clone(&state);
                match thread::Builder::new()
                    .name("rusty-product-dev-connection".to_owned())
                    .spawn(move || {
                        let connection_counter = Arc::clone(&connection_state);
                        let _connection = CounterGuard::new(&connection_counter.connections);
                        handle_connection(stream, connection_state);
                    }) {
                    Ok(thread) => {
                        if let Ok(mut threads) = handler_threads.lock() {
                            threads.push(thread);
                        }
                    }
                    Err(_) => {
                        state.connections.fetch_sub(1, Ordering::AcqRel);
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL)
            }
            Err(_) => break,
        }
    }
}

fn handle_connection<R: ProductDevRuntime>(mut stream: TcpStream, state: Arc<HostState<R>>) {
    if state.shutdown.load(Ordering::Acquire) {
        return;
    }
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(response) => {
            let _ = write_response(&mut stream, response);
            return;
        }
    };
    if !has_admitted_origin(&request, state.bind_host, state.expected_port) {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(
                400,
                "DEV_HOST_ORIGIN",
                "Host or Origin is not this development host origin",
            ),
        );
        return;
    }
    if request.method == "GET" && request.path == format!("{PRODUCT_DEV_RUNTIME_BASE_PATH}outputs")
    {
        handle_sse(stream, state, request);
        return;
    }
    let response = dispatch_request(&state, request);
    let _ = write_response(&mut stream, response);
}

fn dispatch_request<R: ProductDevRuntime>(
    state: &HostState<R>,
    request: HttpRequest,
) -> HttpResponse {
    if request.method == "GET" {
        if request.path == "/__rusty/product/runtime/debug/catalog" {
            if !state.live_debug_enabled {
                return HttpResponse::error(
                    404,
                    "DEV_HOST_ROUTE_NOT_FOUND",
                    "route is not admitted",
                );
            }
            if !request.body.is_empty() {
                return HttpResponse::error(
                    400,
                    "DEV_HOST_GET_BODY",
                    "GET requests cannot carry a body",
                );
            }
            return invoke_debug_catalog(state);
        }
        if request.body.is_empty() {
            if let Some(entry) = state.bundle.get(&request.path) {
                return HttpResponse::bytes(200, entry.content_type(), entry.bytes().to_vec());
            }
            return HttpResponse::error(404, "DEV_HOST_ROUTE_NOT_FOUND", "route is not admitted");
        }
        return HttpResponse::error(400, "DEV_HOST_GET_BODY", "GET requests cannot carry a body");
    }
    if request.method != "POST" {
        return HttpResponse::error(
            405,
            "DEV_HOST_METHOD",
            "route requires its exact admitted method",
        );
    }
    if request.path == "/__rusty/product/runtime/debug/execute" {
        if !state.live_debug_enabled {
            return debug_text_error(404, "live debug route is not admitted");
        }
        if request.headers.get("content-type").map(String::as_str)
            != Some("text/plain; charset=utf-8")
        {
            return debug_text_error(415, "debug execution requires text/plain; charset=utf-8");
        }
        return invoke_debug_execute(state, &request.body);
    }
    if request.headers.get("content-type").map(String::as_str) != Some("application/json") {
        return HttpResponse::error(
            415,
            "DEV_HOST_CONTENT_TYPE",
            "POST requests require application/json",
        );
    }
    match request.path.as_str() {
        "/__rusty/product/runtime/lifecycle/start" => {
            invoke_lifecycle(state, &request.body, ProductDevLifecycleOperation::Start)
        }
        "/__rusty/product/runtime/lifecycle/pause" => {
            invoke_lifecycle(state, &request.body, ProductDevLifecycleOperation::Pause)
        }
        "/__rusty/product/runtime/lifecycle/resume" => {
            invoke_lifecycle(state, &request.body, ProductDevLifecycleOperation::Resume)
        }
        "/__rusty/product/runtime/lifecycle/restart" => {
            invoke_lifecycle(state, &request.body, ProductDevLifecycleOperation::Restart)
        }
        "/__rusty/product/runtime/lifecycle/shutdown" => {
            invoke_lifecycle(state, &request.body, ProductDevLifecycleOperation::Shutdown)
        }
        "/__rusty/product/runtime/lifecycle/report-fault" => invoke_lifecycle(
            state,
            &request.body,
            ProductDevLifecycleOperation::ReportFault,
        ),
        "/__rusty/product/runtime/control/replace" => {
            invoke_control(state, &request.body, ProductDevControlOperation::Replace)
        }
        "/__rusty/product/runtime/control/release" => {
            invoke_control(state, &request.body, ProductDevControlOperation::Release)
        }
        "/__rusty/product/runtime/input" => invoke_input(state, &request.body),
        "/__rusty/product/runtime/advance-realtime" => invoke_realtime(state, &request.body),
        "/__rusty/product/runtime/admit-demand-step" => invoke_demand(state, &request.body),
        "/__rusty/product/runtime/admit-external-step" => invoke_external(state, &request.body),
        "/__rusty/product/runtime/timeline-completion" => invoke_timeline(state, &request.body),
        "/__rusty/product/runtime/audio-feedback" => invoke_audio_feedback(state, &request.body),
        "/__rusty/product/runtime/animation-feedback" => {
            invoke_animation_feedback(state, &request.body)
        }
        _ => HttpResponse::error(404, "DEV_HOST_ROUTE_NOT_FOUND", "route is not admitted"),
    }
}

fn invoke_debug_catalog<R: ProductDevRuntime>(state: &HostState<R>) -> HttpResponse {
    let mut runtime = match state.runtime.lock() {
        Ok(runtime) => runtime,
        Err(_) => {
            return HttpResponse::error(
                500,
                "DEV_HOST_RUNTIME_POISONED",
                "runtime serialization lock is poisoned",
            )
        }
    };
    let receipt = match runtime.describe_debug() {
        Ok(receipt) => receipt,
        Err(error) => return HttpResponse::error(500, error.code(), error.diagnostic()),
    };
    drop(runtime);
    let (catalog, outputs) = receipt.into_parts();
    let output_through = match push_outputs(&state.outputs, outputs) {
        Ok(output_through) => output_through,
        Err(error) => return HttpResponse::error(503, error.code(), error.detail()),
    };
    json_response(200, &catalog).with_output_through(output_through)
}

fn invoke_debug_execute<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    if body.len() > crate::ProductDevDebugResult::MAX_MESSAGE_BYTES {
        return debug_text_error(413, "debug command exceeds host bound");
    }
    let command = match std::str::from_utf8(body) {
        Ok(command) => command,
        Err(_) => return debug_text_error(400, "debug command body must be valid UTF-8"),
    };
    let mut runtime = match state.runtime.lock() {
        Ok(runtime) => runtime,
        Err(_) => return debug_text_error(500, "runtime serialization lock is poisoned"),
    };
    let receipt = match runtime.execute_debug(command) {
        Ok(receipt) => receipt,
        Err(error) => {
            return debug_text_error(500, &format!("{}: {}", error.code(), error.diagnostic()))
        }
    };
    drop(runtime);
    let (result, outputs) = receipt.into_parts();
    let output_through = match push_outputs(&state.outputs, outputs) {
        Ok(output_through) => output_through,
        Err(error) => {
            return debug_text_error(503, &format!("{}: {}", error.code(), error.detail()))
        }
    };
    HttpResponse::text(
        if result.succeeded() { 200 } else { 422 },
        result.message().to_owned(),
    )
    .with_output_through(output_through)
}

fn invoke_lifecycle<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
    operation: ProductDevLifecycleOperation,
) -> HttpResponse {
    let request: LifecycleRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        |runtime| runtime.lifecycle_with_binding(operation, request.runtime),
        |error| {
            ProductDevOperationResult::rejected(
                operation.operation_kind(),
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_control<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
    operation: ProductDevControlOperation,
) -> HttpResponse {
    let request: ControlRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        |runtime| runtime.control(operation, request.runtime),
        |error| {
            ProductDevOperationResult::rejected(
                operation.operation_kind(),
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_input<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: InputRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let batch_json = match serde_json::to_vec(&request.batch) {
        Ok(value) => value,
        Err(_) => {
            return HttpResponse::error(
                400,
                "DEV_HOST_INPUT_DECODE",
                "input batch could not be encoded",
            )
        }
    };
    let events = match decode_runtime_input_wire_events_json(&batch_json) {
        Ok(events) => events,
        Err(_) => {
            return HttpResponse::error(
                400,
                "DEV_HOST_INPUT_DECODE",
                "input batch is not a strict runtime-input wire batch",
            )
        }
    };
    call_runtime(
        state,
        |runtime| runtime.input(ProductDevInputBatch::new(events)),
        |error| {
            crate::ProductDevInputResult::rejected(format!(
                "{}: {}",
                error.code(),
                error.diagnostic()
            ))
        },
    )
}

fn invoke_realtime<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: RealtimeRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        |runtime| runtime.advance_realtime(request.observed_time_ns),
        |error| {
            ProductDevOperationResult::rejected(
                ProductDevOperationKind::AdvanceRealtime,
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_demand<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    if decode_empty(body).is_err() {
        return HttpResponse::error(
            400,
            "DEV_HOST_REQUEST_BODY",
            "demand route requires exactly {} JSON",
        );
    }
    call_runtime(
        state,
        |runtime| runtime.admit_demand_step(),
        |error| {
            ProductDevOperationResult::rejected(
                ProductDevOperationKind::AdmitDemandStep,
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_external<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: ExternalRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        |runtime| runtime.admit_external_step(request.step),
        |error| {
            ProductDevOperationResult::rejected(
                ProductDevOperationKind::AdmitExternalStep,
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_timeline<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: crate::model::ProductDevTimelineCompletionWire = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let request = match ProductDevTimelineCompletion::from_wire(request) {
        Ok(value) => value,
        Err(error) => return HttpResponse::error(400, error.code(), error.detail()),
    };
    let ticket = request.envelope().ticket().value();
    call_runtime(
        state,
        |runtime| runtime.complete_timeline(request),
        |error| {
            crate::ProductDevTimelineCompletionResult::rejected(
                CanonicalU64::new(ticket),
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_audio_feedback<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: crate::ProductDevAudioFeedback = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = request.validate() {
        return HttpResponse::error(400, error.code(), error.detail());
    }
    let binding = request.runtime;
    call_runtime(
        state,
        |runtime| runtime.report_audio_feedback(request),
        |error| {
            crate::ProductDevAudioFeedbackResult::rejected(
                binding,
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn invoke_animation_feedback<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    let request: crate::ProductDevAnimationFeedback = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = request.validate() {
        return HttpResponse::error(400, error.code(), error.detail());
    }
    let binding = request.runtime;
    call_runtime(
        state,
        |runtime| runtime.report_animation_feedback(request),
        |error| {
            crate::ProductDevAnimationFeedbackResult::rejected(
                binding,
                format!("{}: {}", error.code(), error.diagnostic()),
            )
        },
    )
}

fn call_runtime<R, T, F, E>(state: &HostState<R>, call: F, error_result: E) -> HttpResponse
where
    R: ProductDevRuntime,
    T: Serialize,
    F: FnOnce(&mut R) -> Result<crate::ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>,
    E: FnOnce(ProductDevRuntimeError) -> Result<T, ProductDevHostError>,
{
    let mut runtime = match state.runtime.lock() {
        Ok(runtime) => runtime,
        Err(_) => {
            return HttpResponse::error(
                500,
                "DEV_HOST_RUNTIME_POISONED",
                "runtime serialization lock is poisoned",
            )
        }
    };
    let receipt = match call(&mut runtime) {
        Ok(receipt) => receipt,
        Err(error) => {
            return match error_result(error) {
                Ok(result) => json_response(200, &result),
                Err(host_error) => HttpResponse::error(500, host_error.code(), host_error.detail()),
            }
        }
    };
    drop(runtime);
    let (result, outputs) = receipt.into_parts();
    let output_through = match push_outputs(&state.outputs, outputs) {
        Ok(output_through) => output_through,
        Err(error) => return HttpResponse::error(503, error.code(), error.detail()),
    };
    match serde_json::to_vec(&result) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST_BODY_BYTES => {
            HttpResponse::bytes(200, "application/json", bytes).with_output_through(output_through)
        }
        Ok(_) => HttpResponse::error(
            500,
            "DEV_HOST_RESPONSE_BOUNDS",
            "runtime result exceeds response bound",
        ),
        Err(_) => HttpResponse::error(
            500,
            "DEV_HOST_RESPONSE_ENCODE",
            "runtime result could not be encoded",
        ),
    }
}

fn handle_sse<R: ProductDevRuntime>(
    mut stream: TcpStream,
    state: Arc<HostState<R>>,
    request: HttpRequest,
) {
    if request
        .headers
        .get("accept")
        .is_some_and(|value| !value.contains("text/event-stream"))
        || !request.body.is_empty()
    {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(
                400,
                "DEV_HOST_SSE_REQUEST",
                "outputs requires empty EventSource GET",
            ),
        );
        return;
    }
    if !try_acquire(&state.subscribers, MAX_SSE_SUBSCRIBERS) {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(503, "DEV_HOST_SSE_BOUNDS", "SSE subscriber limit reached"),
        );
        return;
    }
    let _subscriber = CounterGuard::new(&state.subscribers);
    let mut cursor = match request.headers.get("last-event-id") {
        Some(value) => match parse_last_event_id(value) {
            Some(value) => value,
            None => {
                let _ = write_response(
                    &mut stream,
                    HttpResponse::error(
                        400,
                        "DEV_HOST_SSE_CURSOR",
                        "Last-Event-ID must be canonical u64 text",
                    ),
                );
                return;
            }
        },
        None => 0,
    };
    if write_sse_headers(&mut stream).is_err() {
        return;
    }
    loop {
        if state.shutdown.load(Ordering::Acquire) {
            break;
        }
        let snapshot = match state.outputs.lock() {
            Ok(outputs) => outputs.after(cursor),
            Err(_) => break,
        };
        if cursor < snapshot.floor_cursor {
            let payload = format!(
                "id: {}\nevent: rusty-output-lag\ndata: {{\"code\":\"DEV_HOST_OUTPUT_LAG\"}}\n\n",
                snapshot.floor_cursor
            );
            let _ = stream.write_all(payload.as_bytes());
            let _ = stream.flush();
            break;
        }
        for event in snapshot.events {
            let payload = match event.event {
                Some(name) => format!(
                    "id: {}\nevent: {}\ndata: {}\n\n",
                    event.id, name, event.json
                ),
                None => format!("id: {}\ndata: {}\n\n", event.id, event.json),
            };
            if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
                return;
            }
            cursor = event.id;
        }
        thread::sleep(SSE_POLL_INTERVAL);
    }
}

#[derive(Default)]
struct OutputBus {
    next_id: u64,
    next_transfer_id: u64,
    events: VecDeque<OutputEvent>,
    floor_cursor: u64,
    active_binding: Option<crate::ProductDevRuntimeBinding>,
    pending_baseline: Option<PendingBaseline>,
}

struct PendingBaseline {
    binding: crate::ProductDevRuntimeBinding,
    outputs: Vec<ProductDevRuntimeOutput>,
}

struct OutputEvent {
    id: u64,
    event: Option<&'static str>,
    json: String,
}

struct OutputSnapshot {
    floor_cursor: u64,
    events: Vec<OutputEvent>,
}

impl OutputBus {
    fn after(&self, cursor: u64) -> OutputSnapshot {
        OutputSnapshot {
            floor_cursor: self.floor_cursor,
            events: self
                .events
                .iter()
                .filter(|event| event.id > cursor)
                .map(|event| OutputEvent {
                    id: event.id,
                    event: event.event,
                    json: event.json.clone(),
                })
                .collect(),
        }
    }
}

fn push_outputs(
    bus: &Mutex<OutputBus>,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<u64, ProductDevHostError> {
    let mut bus = bus.lock().map_err(|_| {
        ProductDevHostError::new("DEV_HOST_OUTPUT_POISONED", "output queue lock is poisoned")
    })?;
    for output in outputs {
        if let Some(binding) = output.binding_marker() {
            if bus.pending_baseline.is_some() {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_BASELINE",
                    "a new binding arrived before the previous baseline completed",
                ));
            }
            bus.pending_baseline = Some(PendingBaseline {
                binding,
                outputs: vec![output],
            });
            continue;
        }
        if let Some(binding) = output.complete_baseline_marker() {
            let (pending_binding, pending_outputs) = {
                let Some(pending) = bus.pending_baseline.as_ref() else {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_OUTPUT_BASELINE",
                        "a baseline completion arrived without its binding",
                    ));
                };
                if pending.binding != binding {
                    return Err(ProductDevHostError::new(
                        "DEV_HOST_OUTPUT_BASELINE",
                        "a baseline completion does not match its binding",
                    ));
                }
                (pending.binding, pending.outputs.clone())
            };
            // Admission is fail-atomic. Once a complete baseline is rejected,
            // discard its staging buffer so the producer can replay the same
            // full binding-to-completion sequence without a phantom baseline.
            if let Err(error) = append_output_events(&mut bus, pending_binding, pending_outputs) {
                bus.pending_baseline = None;
                return Err(error);
            }
            bus.pending_baseline = None;
            bus.active_binding = Some(binding);
            continue;
        }
        if let Some(pending) = &mut bus.pending_baseline {
            pending.outputs.push(output);
            continue;
        }
        if bus.active_binding.is_none() {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BASELINE",
                "incremental output arrived before a complete binding baseline",
            ));
        }
        let binding = bus.active_binding.expect("active binding was checked");
        append_output_events(&mut bus, binding, vec![output])?;
    }
    Ok(bus.next_id)
}

fn append_output_events(
    bus: &mut OutputBus,
    binding: crate::ProductDevRuntimeBinding,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<(), ProductDevHostError> {
    let mut encoded_events = Vec::new();
    let mut next_transfer_id = bus.next_transfer_id;
    for output in outputs {
        let encoded = serde_json::to_string(&output).map_err(|error| {
            ProductDevHostError::new("DEV_HOST_OUTPUT_ENCODE", error.to_string())
        })?;
        if encoded.len() > MAX_OUTPUT_AGGREGATE_BYTES {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BOUNDS",
                "output aggregate exceeds host bound",
            ));
        }
        if encoded.len() <= MAX_OUTPUT_EVENT_BYTES {
            encoded_events.push(EncodedOutputEvent {
                event: None,
                json: encoded,
            });
            continue;
        }
        next_transfer_id = next_transfer_id.checked_add(1).ok_or_else(|| {
            ProductDevHostError::new("DEV_HOST_OUTPUT_ID", "output transfer sequence exhausted")
        })?;
        let slices = fragment_slices(&encoded);
        for (fragment_index, data) in slices.iter().enumerate() {
            let fragment = OutputFragment {
                schema_version: 1,
                transfer_id: CanonicalU64::new(next_transfer_id),
                runtime: binding,
                fragment_index,
                fragment_count: slices.len(),
                aggregate_bytes: encoded.len(),
                data,
            };
            let json = serde_json::to_string(&fragment).map_err(|error| {
                ProductDevHostError::new("DEV_HOST_OUTPUT_ENCODE", error.to_string())
            })?;
            if json.len() > MAX_OUTPUT_EVENT_BYTES {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_BOUNDS",
                    "output fragment exceeds host event bound",
                ));
            }
            encoded_events.push(EncodedOutputEvent {
                event: Some("rusty-output-fragment"),
                json,
            });
        }
    }
    if encoded_events.len() > MAX_OUTPUT_QUEUE_ITEMS {
        return Err(ProductDevHostError::new(
            "DEV_HOST_OUTPUT_BATCH_BOUNDS",
            "encoded output batch exceeds the retained event count",
        ));
    }
    let final_id = bus
        .next_id
        .checked_add(encoded_events.len() as u64)
        .ok_or_else(|| {
            ProductDevHostError::new("DEV_HOST_OUTPUT_ID", "output sequence exhausted")
        })?;
    for encoded in encoded_events {
        bus.next_id += 1;
        if bus.events.len() == MAX_OUTPUT_QUEUE_ITEMS {
            if let Some(evicted) = bus.events.pop_front() {
                bus.floor_cursor = evicted.id;
            }
        }
        bus.events.push_back(OutputEvent {
            id: bus.next_id,
            event: encoded.event,
            json: encoded.json,
        });
    }
    debug_assert_eq!(bus.next_id, final_id);
    bus.next_transfer_id = next_transfer_id;
    Ok(())
}

struct EncodedOutputEvent {
    event: Option<&'static str>,
    json: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputFragment<'a> {
    schema_version: u8,
    transfer_id: CanonicalU64,
    runtime: crate::ProductDevRuntimeBinding,
    fragment_index: usize,
    fragment_count: usize,
    aggregate_bytes: usize,
    data: &'a str,
}

fn fragment_slices(encoded: &str) -> Vec<&str> {
    let mut slices = Vec::new();
    let mut start = 0;
    while start < encoded.len() {
        let mut end = (start + MAX_OUTPUT_FRAGMENT_DATA_BYTES).min(encoded.len());
        while !encoded.is_char_boundary(end) {
            end -= 1;
        }
        slices.push(&encoded[start..end]);
        start = end;
    }
    slices
}

struct CounterGuard<'a> {
    counter: &'a AtomicUsize,
}

impl<'a> CounterGuard<'a> {
    const fn new(counter: &'a AtomicUsize) -> Self {
        Self { counter }
    }
}

impl Drop for CounterGuard<'_> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

fn try_acquire(counter: &AtomicUsize, maximum: usize) -> bool {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            (current < maximum).then_some(current + 1)
        })
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> crate::ProductDevRuntimeBinding {
        crate::ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(2),
        }
    }

    #[test]
    fn oversized_output_uses_bounded_ordered_fragments_while_small_output_stays_simple() {
        let mut bus = OutputBus::default();
        append_output_events(
            &mut bus,
            binding(),
            vec![crate::model::ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"payload": "small"}),
            )],
        )
        .unwrap();
        assert_eq!(bus.events.len(), 1);
        assert_eq!(bus.events[0].event, None);

        append_output_events(
            &mut bus,
            binding(),
            vec![crate::model::ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"payload": "x".repeat(MAX_OUTPUT_EVENT_BYTES + 1)}),
            )],
        )
        .unwrap();
        let fragments = bus.events.iter().skip(1).collect::<Vec<_>>();
        assert!(fragments.len() > 1);
        assert!(fragments.iter().all(|event| {
            event.event == Some("rusty-output-fragment")
                && event.json.len() <= MAX_OUTPUT_EVENT_BYTES
        }));
        let decoded = fragments
            .iter()
            .map(|event| serde_json::from_str::<serde_json::Value>(&event.json).unwrap())
            .collect::<Vec<_>>();
        assert!(decoded.iter().enumerate().all(|(index, value)| {
            value["fragmentIndex"] == index
                && value["fragmentCount"] == fragments.len()
                && value["runtime"]["generation"] == "1"
        }));
    }

    #[test]
    fn oversized_aggregate_is_rejected_without_publishing_partial_events() {
        let mut bus = OutputBus::default();
        let error = append_output_events(
            &mut bus,
            binding(),
            vec![crate::model::ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"payload": "x".repeat(MAX_OUTPUT_AGGREGATE_BYTES + 1)}),
            )],
        )
        .unwrap_err();
        assert_eq!(error.code(), "DEV_HOST_OUTPUT_BOUNDS");
        assert!(bus.events.is_empty());
        assert_eq!(bus.next_id, 0);
        assert_eq!(bus.next_transfer_id, 0);
    }

    #[test]
    fn rejected_fragmented_baseline_accepts_a_real_full_producer_replay() {
        let runtime = binding();
        let fragmented_payload =
            "x".repeat(MAX_OUTPUT_FRAGMENT_DATA_BYTES * (MAX_OUTPUT_QUEUE_ITEMS / 2 + 1));
        let bus = Mutex::new(OutputBus::default());
        let error = push_outputs(
            &bus,
            vec![
                crate::model::ProductDevRuntimeOutput::binding(runtime, CanonicalU64::new(0)),
                crate::model::ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"payload": fragmented_payload}),
                ),
                crate::model::ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"payload": "y".repeat(
                        MAX_OUTPUT_FRAGMENT_DATA_BYTES * (MAX_OUTPUT_QUEUE_ITEMS / 2 + 1),
                    )}),
                ),
                crate::model::ProductDevRuntimeOutput::complete_baseline(runtime),
            ],
        )
        .expect_err("oversized complete producer baseline is rejected");
        assert_eq!(error.code(), "DEV_HOST_OUTPUT_BATCH_BOUNDS");
        assert!(bus
            .lock()
            .expect("test bus lock")
            .pending_baseline
            .is_none());
        let incremental = push_outputs(
            &bus,
            vec![crate::model::ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"incremental": true}),
            )],
        )
        .expect_err("incremental output cannot attach to a rejected baseline");
        assert_eq!(incremental.code(), "DEV_HOST_OUTPUT_BASELINE");
        push_outputs(
            &bus,
            vec![
                crate::model::ProductDevRuntimeOutput::binding(runtime, CanonicalU64::new(0)),
                crate::model::ProductDevRuntimeOutput::complete_baseline(runtime),
            ],
        )
        .expect("a replayed full producer baseline publishes atomically");
        let locked = bus.lock().expect("test bus lock");
        assert!(locked.pending_baseline.is_none());
        assert_eq!(locked.active_binding, Some(runtime));
        assert_eq!(locked.events.len(), 1);
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, HttpResponse> {
    stream.set_read_timeout(Some(SOCKET_TIMEOUT)).map_err(|_| {
        HttpResponse::error(
            500,
            "DEV_HOST_SOCKET",
            "could not configure request timeout",
        )
    })?;
    let mut bytes = Vec::with_capacity(1024);
    let header_end;
    loop {
        let mut buffer = [0_u8; 1024];
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(HttpResponse::error(
                    400,
                    "DEV_HOST_REQUEST_EOF",
                    "request ended before headers",
                ))
            }
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > MAX_REQUEST_HEADER_BYTES + MAX_REQUEST_BODY_BYTES {
                    return Err(HttpResponse::error(
                        413,
                        "DEV_HOST_REQUEST_BOUNDS",
                        "request exceeds host byte limit",
                    ));
                }
                if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    header_end = index + 4;
                    break;
                }
                if bytes.len() > MAX_REQUEST_HEADER_BYTES {
                    return Err(HttpResponse::error(
                        431,
                        "DEV_HOST_HEADER_BOUNDS",
                        "request headers exceed host bound",
                    ));
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(HttpResponse::error(
                    408,
                    "DEV_HOST_REQUEST_TIMEOUT",
                    "request header timeout",
                ));
            }
            Err(_) => {
                return Err(HttpResponse::error(
                    400,
                    "DEV_HOST_REQUEST_READ",
                    "could not read request",
                ))
            }
        }
    }
    let head = std::str::from_utf8(&bytes[..header_end - 4]).map_err(|_| {
        HttpResponse::error(400, "DEV_HOST_HEADER_UTF8", "request headers must be ASCII")
    })?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| {
        HttpResponse::error(400, "DEV_HOST_REQUEST_LINE", "request line is required")
    })?;
    let mut request_parts = request_line.split_ascii_whitespace();
    let method = request_parts.next().unwrap_or_default();
    let path = request_parts.next().unwrap_or_default();
    let protocol = request_parts.next().unwrap_or_default();
    if request_parts.next().is_some()
        || !matches!(method, "GET" | "POST")
        || protocol != "HTTP/1.1"
        || !valid_request_path(path)
    {
        return Err(HttpResponse::error(
            400,
            "DEV_HOST_REQUEST_LINE",
            "request line is not admitted",
        ));
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(HttpResponse::error(
                400,
                "DEV_HOST_HEADER",
                "request header is malformed",
            ));
        };
        let name = name.to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name.is_empty()
            || headers.insert(name.clone(), value).is_some()
            || name == "transfer-encoding"
        {
            return Err(HttpResponse::error(
                400,
                "DEV_HOST_HEADER",
                "request header is not admitted",
            ));
        }
    }
    if headers
        .get("connection")
        .is_some_and(|value| value.to_ascii_lowercase().contains("upgrade"))
    {
        return Err(HttpResponse::error(
            400,
            "DEV_HOST_CONNECTION",
            "connection upgrades are not admitted",
        ));
    }
    let content_length = match headers.get("content-length") {
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|length| *length <= MAX_REQUEST_BODY_BYTES),
        None => Some(0),
    }
    .ok_or_else(|| {
        HttpResponse::error(413, "DEV_HOST_BODY_BOUNDS", "body length is not admitted")
    })?;
    let mut body = bytes[header_end..].to_vec();
    if body.len() > content_length {
        return Err(HttpResponse::error(
            400,
            "DEV_HOST_BODY_LENGTH",
            "request has trailing bytes",
        ));
    }
    while body.len() < content_length {
        let mut buffer = [0_u8; 1024];
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(HttpResponse::error(
                    400,
                    "DEV_HOST_BODY_EOF",
                    "request ended before body",
                ))
            }
            Ok(count) => body.extend_from_slice(&buffer[..count]),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(HttpResponse::error(
                    408,
                    "DEV_HOST_BODY_TIMEOUT",
                    "request body timeout",
                ));
            }
            Err(_) => {
                return Err(HttpResponse::error(
                    400,
                    "DEV_HOST_BODY_READ",
                    "could not read request body",
                ))
            }
        }
    }
    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    })
}

fn valid_request_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 512
        && !path.contains('?')
        && !path.contains('#')
        && !path.contains("//")
        && !path.split('/').any(|part| part == "." || part == "..")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_'))
}

fn has_admitted_origin(request: &HttpRequest, bind_host: Ipv4Addr, expected_port: u16) -> bool {
    let Some(host) = request.headers.get("host") else {
        return false;
    };
    let host_admitted = if bind_host.is_loopback() {
        host == &format!("{bind_host}:{expected_port}")
    } else {
        host.rsplit_once(':').is_some_and(|(name, port)| {
            !name.is_empty() && port.parse::<u16>() == Ok(expected_port)
        })
    };
    host_admitted
        && request
            .headers
            .get("origin")
            .is_none_or(|origin| origin == &format!("http://{host}"))
}

struct HttpResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    output_through: Option<u64>,
}

impl HttpResponse {
    fn bytes(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type,
            body,
            output_through: None,
        }
    }

    fn with_output_through(mut self, output_through: u64) -> Self {
        self.output_through = Some(output_through);
        self
    }

    fn text(status: u16, text: String) -> Self {
        Self::bytes(
            status,
            "text/plain; charset=utf-8",
            bounded_text(&text, 64 * 1024).into_bytes(),
        )
    }

    fn error(status: u16, code: &str, detail: &str) -> Self {
        let code = bounded_text(code, 128);
        let detail = bounded_text(detail, 512);
        let body = format!(
            "{{\"accepted\":false,\"error\":{{\"code\":{},\"diagnostic\":{}}}}}",
            json_string(&code),
            json_string(&detail)
        )
        .into_bytes();
        Self::bytes(status, "application/json", body)
    }
}

fn debug_text_error(status: u16, detail: &str) -> HttpResponse {
    HttpResponse::text(status, detail.to_owned())
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> io::Result<()> {
    stream.set_write_timeout(Some(SOCKET_TIMEOUT))?;
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Content",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(stream, "HTTP/1.1 {} {}\r\n", response.status, reason)?;
    write!(stream, "Content-Type: {}\r\n", response.content_type)?;
    write!(stream, "Content-Length: {}\r\n", response.body.len())?;
    if let Some(output_through) = response.output_through {
        write!(stream, "X-Rusty-Output-Through: {output_through}\r\n")?;
    }
    stream.write_all(
        b"Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
    )?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn write_sse_headers(stream: &mut TcpStream) -> io::Result<()> {
    stream.set_write_timeout(Some(SOCKET_TIMEOUT))?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n")?;
    stream.flush()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyRequest {}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LifecycleRequest {
    #[serde(default)]
    runtime: Option<crate::ProductDevRuntimeBinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ControlRequest {
    runtime: crate::ProductDevRuntimeBinding,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct InputRequest {
    batch: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RealtimeRequest {
    observed_time_ns: CanonicalU64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExternalRequest {
    step: CanonicalU64,
}

fn decode_empty(body: &[u8]) -> Result<(), HttpResponse> {
    decode_json::<EmptyRequest>(body).map(|_| ())
}

fn decode_json<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, HttpResponse> {
    if body.len() > MAX_REQUEST_BODY_BYTES {
        return Err(HttpResponse::error(
            413,
            "DEV_HOST_BODY_BOUNDS",
            "request body exceeds host bound",
        ));
    }
    serde_json::from_slice(body).map_err(|_| {
        HttpResponse::error(
            400,
            "DEV_HOST_JSON",
            "request JSON is malformed or has unknown fields",
        )
    })
}

fn json_response<T: Serialize>(status: u16, value: &T) -> HttpResponse {
    match serde_json::to_vec(value) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST_BODY_BYTES => {
            HttpResponse::bytes(status, "application/json", bytes)
        }
        _ => HttpResponse::error(
            500,
            "DEV_HOST_RESPONSE_ENCODE",
            "response could not be encoded",
        ),
    }
}

fn parse_last_event_id(value: &str) -> Option<u64> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

fn bounded_text(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

fn reap_finished_handlers(handlers: &Mutex<Vec<JoinHandle<()>>>) {
    let Ok(mut handlers) = handlers.lock() else {
        return;
    };
    let mut live = Vec::with_capacity(handlers.len());
    for handle in std::mem::take(&mut *handlers) {
        if handle.is_finished() {
            let _ = handle.join();
        } else {
            live.push(handle);
        }
    }
    *handlers = live;
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"encoding failure\"".to_owned())
}
