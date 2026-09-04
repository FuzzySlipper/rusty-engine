use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::{
    CanonicalU64, ProductDevBrowserConnectionState, ProductDevBrowserDiagnosticsReport,
    ProductDevBrowserDiagnosticsResult, ProductDevBrowserHostState, ProductDevBundle,
    ProductDevControlOperation, ProductDevHostError, ProductDevInputBatch, ProductDevInputResult,
    ProductDevLifecycleOperation, ProductDevLog, ProductDevLogDisposition, ProductDevLogEvent,
    ProductDevLogSeverity, ProductDevNextAction, ProductDevOperationKind,
    ProductDevOperationResult, ProductDevRuntime, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevRuntimeReceipt, ProductDevTelemetrySnapshot, ProductDevTimelineCompletion,
    ProductDevUpdateAttribution, ProductDevUpdateAttributionSnapshot, ProductDevWorkerDiagnostic,
    ProductDevWorkerOutputBatch, MAX_CONNECTIONS, MAX_OUTPUT_AGGREGATE_BYTES,
    MAX_OUTPUT_EVENT_BYTES, MAX_OUTPUT_FRAGMENT_DATA_BYTES, MAX_OUTPUT_QUEUE_ITEMS,
    MAX_REQUEST_BODY_BYTES, MAX_REQUEST_HEADER_BYTES, MAX_SSE_SUBSCRIBERS,
};

use crate::session::ProductDevOperationOwner;

const SOCKET_TIMEOUT: Duration = Duration::from_millis(750);
const SSE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
const ACCEPT_RETRY_INITIAL_BACKOFF: Duration = Duration::from_millis(5);
const ACCEPT_RETRY_MAX_BACKOFF: Duration = Duration::from_millis(200);
const SCHEDULER_IDLE_WAIT: Duration = Duration::from_millis(100);
/// Maximum number of transport batches retained between Rust-host realtime
/// observations. Each batch is independently bounded by runtime-input's wire
/// event limit; the host never grows this queue to match renderer cadence.
pub const MAX_HOST_INPUT_BATCHES: usize = 256;

/// A deliberately narrow test seam for one listener accept decision. It is
/// not a transport abstraction: production always invokes `TcpListener`.
type AcceptDecisionHook = Arc<dyn Fn() -> Option<io::ErrorKind> + Send + Sync>;
type WorkerOutputReceiver = Arc<Mutex<mpsc::Receiver<ProductDevWorkerOutputBatch>>>;
type WorkerDiagnosticReceiver = Arc<Mutex<mpsc::Receiver<ProductDevWorkerDiagnostic>>>;

/// Configuration for the fixed development host.
#[derive(Clone)]
pub struct ProductDevHostConfig {
    /// `0` asks the operating system for a free port.
    pub port: u16,
    pub bundle: ProductDevBundle,
    bind_host: Ipv4Addr,
    live_debug_enabled: bool,
    diagnostics: ProductDevLog,
    accept_decision_hook: Option<AcceptDecisionHook>,
    worker_outputs: Option<WorkerOutputReceiver>,
    worker_generation: Option<Arc<AtomicUsize>>,
    initial_worker_outputs: Vec<ProductDevRuntimeOutput>,
    initial_worker_generation: u64,
    worker_failures: Option<mpsc::SyncSender<u64>>,
    worker_diagnostics: Option<WorkerDiagnosticReceiver>,
    worker_owns_scheduler: bool,
    disposable_worker_runtime: bool,
}

impl ProductDevHostConfig {
    pub fn new(port: u16, bundle: ProductDevBundle) -> Self {
        Self {
            port,
            bundle,
            bind_host: Ipv4Addr::LOCALHOST,
            live_debug_enabled: false,
            diagnostics: ProductDevLog::new(Default::default()).expect("fixed diagnostic defaults"),
            accept_decision_hook: None,
            worker_outputs: None,
            worker_generation: None,
            initial_worker_outputs: Vec::new(),
            initial_worker_generation: 0,
            worker_failures: None,
            worker_diagnostics: None,
            worker_owns_scheduler: false,
            disposable_worker_runtime: false,
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

    pub fn with_diagnostics(mut self, diagnostics: ProductDevLog) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Supplies retained output from one disposable local worker. The reader
    /// is intentionally one-way: requests remain on the typed runtime owner.
    pub fn with_worker_outputs(
        mut self,
        receiver: mpsc::Receiver<ProductDevWorkerOutputBatch>,
        generation: Arc<AtomicUsize>,
        failures: mpsc::SyncSender<u64>,
        initial_outputs: Vec<ProductDevRuntimeOutput>,
        initial_generation: u64,
    ) -> Self {
        self.worker_outputs = Some(Arc::new(Mutex::new(receiver)));
        self.worker_generation = Some(generation);
        self.initial_worker_outputs = initial_outputs;
        self.initial_worker_generation = initial_generation;
        self.worker_failures = Some(failures);
        self
    }

    /// Supplies bounded diagnostic facts from one disposable local worker.
    /// The stable shell owns their retained/log-file projection just as it
    /// owns HTTP and SSE; worker stderr remains human output only.
    pub fn with_worker_diagnostics(
        mut self,
        receiver: mpsc::Receiver<ProductDevWorkerDiagnostic>,
    ) -> Self {
        self.worker_diagnostics = Some(Arc::new(Mutex::new(receiver)));
        self
    }

    /// Marks the runtime scheduler as worker-owned. The stable HTTP shell
    /// then does not create even an idle local scheduler thread.
    pub fn with_worker_scheduler(mut self) -> Self {
        self.worker_owns_scheduler = true;
        self.disposable_worker_runtime = true;
        self
    }

    /// Injects synthetic listener errors for the loopback-host integration
    /// proof. Returning `None` delegates directly to the real listener.
    #[doc(hidden)]
    pub fn with_test_accept_decision_hook(
        mut self,
        hook: impl Fn() -> Option<io::ErrorKind> + Send + Sync + 'static,
    ) -> Self {
        self.accept_decision_hook = Some(Arc::new(hook));
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
        let address = listener
            .local_addr()
            .map_err(|error| ProductDevHostError::io("DEV_HOST_ADDRESS", error))?;
        // Capture only the runtime's scheduler participation, not its
        // mutable lifecycle state. Input admission must never reacquire the
        // runtime owner behind a slow product update; Created/Paused realtime
        // products still retain their bounded mailbox for the next boundary.
        let realtime_scheduler_enabled = !matches!(
            runtime.realtime_schedule_state(),
            crate::ProductDevRuntimeScheduleState::Unsupported
        );
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler_wake = Arc::new(SchedulerWake::default());
        let output_wake = Arc::new(OutputWake::default());
        let bundle = Arc::new(RwLock::new(config.bundle));
        let mut initial_output_bus = OutputBus::default();
        push_outputs_staged(&mut initial_output_bus, config.initial_worker_outputs)?;
        let outputs = Arc::new(Mutex::new(initial_output_bus));
        let projection_gate = Arc::new(RwLock::new(()));
        let state = Arc::new(HostState {
            bundle: Arc::clone(&bundle),
            runtime: Arc::new(ProductDevOperationOwner::new(runtime)),
            input_mailbox: Arc::new(HostInputMailbox::default()),
            telemetry: Mutex::new(HostTelemetry::default()),
            realtime_scheduler_enabled,
            disposable_worker_runtime: config.disposable_worker_runtime,
            outputs: Arc::clone(&outputs),
            output_wake: Arc::clone(&output_wake),
            shutdown: Arc::clone(&shutdown),
            scheduler_wake: Arc::clone(&scheduler_wake),
            bind_host: config.bind_host,
            expected_port: address.port(),
            live_debug_enabled: config.live_debug_enabled,
            diagnostics: config.diagnostics.clone(),
            projection_gate: Arc::clone(&projection_gate),
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
        });
        let handler_threads = Arc::new(Mutex::new(Vec::new()));
        let worker_outputs = config.worker_outputs;
        let worker_generation = config.worker_generation;
        if let Some(generation) = &worker_generation {
            generation.store(config.initial_worker_generation as usize, Ordering::Release);
        }
        let worker_output_generation = worker_generation.clone();
        let worker_failures = config.worker_failures;
        let worker_diagnostics = config.worker_diagnostics;
        let listener_state = Arc::clone(&state);
        let listener_threads = Arc::clone(&handler_threads);
        let accept_decision_hook = config.accept_decision_hook;
        let listener_thread = thread::Builder::new()
            .name("rusty-product-dev-host".to_owned())
            .spawn(move || {
                accept_loop(
                    listener,
                    listener_state,
                    listener_threads,
                    accept_decision_hook,
                )
            })
            .map_err(|error| ProductDevHostError::io("DEV_HOST_THREAD", error))?;
        let scheduler_state = Arc::clone(&state);
        let scheduler_wake_thread = Arc::clone(&scheduler_wake);
        let scheduler_thread = if config.worker_owns_scheduler {
            None
        } else {
            match thread::Builder::new()
                .name("rusty-product-realtime-scheduler".to_owned())
                .spawn(move || scheduler_loop(scheduler_state, scheduler_wake_thread))
            {
                Ok(thread) => Some(thread),
                Err(error) => {
                    // The listener already owns a live socket at this point. If
                    // scheduler creation fails, close that ownership explicitly
                    // before returning so a half-started host cannot survive.
                    shutdown.store(true, Ordering::SeqCst);
                    scheduler_wake.notify();
                    output_wake.notify();
                    let _ = TcpStream::connect_timeout(&address, SOCKET_TIMEOUT);
                    let _ = listener_thread.join();
                    if let Ok(mut handlers) = handler_threads.lock() {
                        for handler in std::mem::take(&mut *handlers) {
                            let _ = handler.join();
                        }
                    }
                    return Err(ProductDevHostError::io("DEV_HOST_THREAD", error));
                }
            }
        };
        let worker_output_thread = worker_outputs.map(|receiver| {
            let outputs = Arc::clone(&outputs);
            let wake = Arc::clone(&output_wake);
            let shutdown = Arc::clone(&shutdown);
            let projection_gate = Arc::clone(&projection_gate);
            let generation =
                worker_output_generation.expect("worker output generation accompanies receiver");
            let failures = worker_failures.expect("worker output failures accompany receiver");
            let diagnostics = config.diagnostics.clone();
            thread::Builder::new()
                .name("rusty-product-worker-output".to_owned())
                .spawn(move || {
                    while !shutdown.load(Ordering::Acquire) {
                        let batch = match receiver.lock() {
                            Ok(receiver) => receiver.recv_timeout(Duration::from_millis(50)),
                            Err(_) => return,
                        };
                        match batch {
                            Ok(outputs_from_worker) => {
                                let _projection = match projection_gate.read() {
                                    Ok(gate) => gate,
                                    Err(_) => return,
                                };
                                if outputs_from_worker.generation
                                    != generation.load(Ordering::Acquire) as u64
                                {
                                    continue;
                                }
                                let worker_generation = outputs_from_worker.generation;
                                match push_outputs(&outputs, outputs_from_worker.outputs) {
                                    Ok(_) => wake.notify(),
                                    Err(error) => {
                                        generation.store(0, Ordering::Release);
                                        publish_host_diagnostic(
                                            &diagnostics,
                                            ProductDevLogSeverity::Error,
                                            ProductDevLogDisposition::Degraded,
                                            error.code(),
                                            error.detail(),
                                            [("worker-generation", worker_generation.to_string())],
                                        );
                                        let _ = failures.try_send(worker_generation);
                                    }
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        }
                    }
                })
                .expect("worker output thread creation")
        });
        let worker_diagnostic_thread = worker_diagnostics.map(|receiver| {
            let diagnostics = config.diagnostics.clone();
            let shutdown = Arc::clone(&shutdown);
            thread::Builder::new()
                .name("rusty-product-worker-diagnostic".to_owned())
                .spawn(move || {
                    while !shutdown.load(Ordering::Acquire) {
                        let error = match receiver.lock() {
                            Ok(receiver) => receiver.recv_timeout(Duration::from_millis(50)),
                            Err(_) => return,
                        };
                        match error {
                            Ok(diagnostic) => {
                                if let Ok(event) = diagnostic.into_log_event() {
                                    let _ = diagnostics.publish(event);
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => return,
                        }
                    }
                })
                .expect("worker diagnostic thread creation")
        });
        Ok(RunningProductDevHost {
            address,
            shutdown,
            scheduler_wake,
            output_wake,
            scheduler_thread,
            listener_thread: Some(listener_thread),
            handler_threads,
            diagnostics: config.diagnostics,
            bundle,
            outputs,
            projection_gate,
            worker_generation,
            worker_output_thread,
            worker_diagnostic_thread,
        })
    }
}

/// A running development host. Shutdown is explicit and joins every accepted
/// connection handler so tests and generated launchers do not leak threads.
pub struct RunningProductDevHost {
    address: SocketAddr,
    shutdown: Arc<AtomicBool>,
    scheduler_wake: Arc<SchedulerWake>,
    output_wake: Arc<OutputWake>,
    scheduler_thread: Option<JoinHandle<()>>,
    listener_thread: Option<JoinHandle<()>>,
    handler_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    diagnostics: ProductDevLog,
    bundle: Arc<RwLock<ProductDevBundle>>,
    outputs: Arc<Mutex<OutputBus>>,
    projection_gate: Arc<RwLock<()>>,
    worker_generation: Option<Arc<AtomicUsize>>,
    worker_output_thread: Option<JoinHandle<()>>,
    worker_diagnostic_thread: Option<JoinHandle<()>>,
}

impl RunningProductDevHost {
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    /// A terminal runtime recovery asks the foreground product-host process to
    /// leave this incarnation. The outer `rusty dev` supervisor remains the
    /// owner of replacement and observes the resulting child exit.
    pub fn termination_requested(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn shutdown(mut self) -> Result<(), ProductDevHostError> {
        self.stop()
    }

    /// Atomically makes a freshly loaded worker visible to the stable browser
    /// shell. A changed binding fences old retained output before the new
    /// complete baseline is committed, so neither live SSE clients nor an old
    /// Last-Event-ID reconnect can consume facts from the retired incarnation.
    pub fn replace_worker_projection(
        &self,
        bundle: ProductDevBundle,
        outputs: Vec<ProductDevRuntimeOutput>,
        worker_generation: u64,
    ) -> Result<u64, ProductDevHostError> {
        let _gate = self.projection_gate.write().map_err(|_| {
            ProductDevHostError::new(
                "DEV_HOST_WORKER_REPLACE",
                "projection replacement lock is poisoned",
            )
        })?;
        let new_binding = outputs
            .iter()
            .find_map(ProductDevRuntimeOutput::binding_marker);
        {
            let mut bus = self.outputs.lock().map_err(|_| {
                ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_POISONED",
                    "output queue lock is poisoned",
                )
            })?;
            let mut current = self.bundle.write().map_err(|_| {
                ProductDevHostError::new(
                    "DEV_HOST_BUNDLE_REPLACE",
                    "bundle replacement lock is poisoned",
                )
            })?;
            if new_binding.is_some_and(|binding| Some(binding) != bus.active_binding) {
                bus.events.clear();
                bus.floor_cursor = bus.next_id;
                bus.active_binding = None;
                bus.pending_baseline = None;
            }
            let through = push_outputs_staged(&mut bus, outputs)?;
            *current = bundle;
            if let Some(generation) = &self.worker_generation {
                generation.store(worker_generation as usize, Ordering::Release);
            }
            self.output_wake.notify();
            return Ok(through);
        }
    }

    fn stop(&mut self) -> Result<(), ProductDevHostError> {
        let was_shutdown = self.shutdown.swap(true, Ordering::SeqCst);
        // Stop the host-owned realtime loop first. It may be in a product
        // callback, so joining it before connection handlers/runtime drop
        // preserves one clear teardown order and prevents post-shutdown work.
        self.scheduler_wake.notify();
        self.output_wake.notify();
        if let Some(thread) = self.scheduler_thread.take() {
            thread.join().map_err(|_| {
                ProductDevHostError::new("DEV_HOST_THREAD_JOIN", "scheduler thread panicked")
            })?;
        }
        if let Some(thread) = self.worker_output_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.worker_diagnostic_thread.take() {
            let _ = thread.join();
        }
        // Wake a nonblocking accept loop promptly. The connection is accepted
        // and observes the same shutdown flag before it parses a request.
        if !was_shutdown || self.listener_thread.is_some() {
            let _ = TcpStream::connect_timeout(&self.address, SOCKET_TIMEOUT);
        }
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
        self.diagnostics.flush();
        Ok(())
    }
}

impl Drop for RunningProductDevHost {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct HostState<R> {
    bundle: Arc<RwLock<ProductDevBundle>>,
    runtime: Arc<ProductDevOperationOwner<R>>,
    input_mailbox: Arc<HostInputMailbox>,
    telemetry: Mutex<HostTelemetry>,
    realtime_scheduler_enabled: bool,
    disposable_worker_runtime: bool,
    outputs: Arc<Mutex<OutputBus>>,
    output_wake: Arc<OutputWake>,
    shutdown: Arc<AtomicBool>,
    scheduler_wake: Arc<SchedulerWake>,
    bind_host: Ipv4Addr,
    expected_port: u16,
    live_debug_enabled: bool,
    diagnostics: ProductDevLog,
    projection_gate: Arc<RwLock<()>>,
    connections: AtomicUsize,
    subscribers: AtomicUsize,
}

/// Small process-local observation state. It intentionally has no runtime
/// reference: diagnostics can read it while the product owner is in a slow
/// callback and therefore never become another source of input backpressure.
#[derive(Default)]
struct HostTelemetry {
    in_flight_operation: Option<ProductDevOperationKind>,
    in_flight_started_ns: Option<u64>,
    last_product_admission_latency_ms: Option<u64>,
    last_input_admission_latency_ms: Option<u64>,
    progress_samples_ns: VecDeque<u64>,
    update_attribution_samples: VecDeque<(u64, ProductDevUpdateAttribution)>,
    slowest_update_attribution: Option<(u64, ProductDevUpdateAttribution)>,
}

impl HostTelemetry {
    const MAX_PROGRESS_SAMPLES: usize = 32;
    const PROGRESS_WINDOW_NS: u64 = 5_000_000_000;
    const MAX_UPDATE_ATTRIBUTION_SAMPLES: usize = 2_048;

    fn begin(&mut self, operation: ProductDevOperationKind, started_ns: u64) {
        self.in_flight_operation = Some(operation);
        self.in_flight_started_ns = Some(started_ns);
    }

    fn finish(&mut self, finished_ns: u64) -> Option<u64> {
        let latency_ms = self.in_flight_started_ns.take().map(|started_ns| {
            finished_ns
                .saturating_sub(started_ns)
                .saturating_div(1_000_000)
        });
        if let Some(latency_ms) = latency_ms {
            self.last_product_admission_latency_ms = Some(latency_ms);
        }
        self.in_flight_operation = None;
        latency_ms
    }

    fn record_input_admission(&mut self, latency_ms: u64) {
        self.last_input_admission_latency_ms = Some(latency_ms);
    }

    fn record_progress(&mut self, now_ns: u64) {
        self.progress_samples_ns.push_back(now_ns);
        while self.progress_samples_ns.len() > Self::MAX_PROGRESS_SAMPLES {
            self.progress_samples_ns.pop_front();
        }
        while self
            .progress_samples_ns
            .front()
            .is_some_and(|sample| now_ns.saturating_sub(*sample) > Self::PROGRESS_WINDOW_NS)
        {
            self.progress_samples_ns.pop_front();
        }
    }

    fn record_update_attribution(
        &mut self,
        completed_ns: u64,
        sample: ProductDevUpdateAttribution,
    ) {
        self.update_attribution_samples
            .push_back((completed_ns, sample));
        while self.update_attribution_samples.len() > Self::MAX_UPDATE_ATTRIBUTION_SAMPLES {
            self.update_attribution_samples.pop_front();
        }
        if self.slowest_update_attribution.is_none_or(|(_, slowest)| {
            sample.callback_duration_us.get() > slowest.callback_duration_us.get()
        }) {
            self.slowest_update_attribution = Some((completed_ns, sample));
        }
    }

    fn update_attribution_snapshot(
        &self,
        now_ns: u64,
    ) -> Option<ProductDevUpdateAttributionSnapshot> {
        let (_, latest) = *self.update_attribution_samples.back()?;
        let mut durations = self
            .update_attribution_samples
            .iter()
            .map(|(_, sample)| sample.callback_duration_us.get())
            .collect::<Vec<_>>();
        durations.sort_unstable();
        let percentile = |numerator: usize, denominator: usize| {
            durations[(durations.len().saturating_sub(1)).saturating_mul(numerator) / denominator]
        };
        let (rolling_slowest_ns, rolling_slowest) = self
            .update_attribution_samples
            .iter()
            .copied()
            .max_by_key(|(_, sample)| sample.callback_duration_us.get())
            .expect("a non-empty attribution window has a slowest sample");
        let (slowest_ns, slowest) = self.slowest_update_attribution.unwrap_or((now_ns, latest));
        Some(ProductDevUpdateAttributionSnapshot {
            sample_count: CanonicalU64::new(self.update_attribution_samples.len() as u64),
            callback_duration_us_p50: CanonicalU64::new(percentile(50, 100)),
            callback_duration_us_p95: CanonicalU64::new(percentile(95, 100)),
            callback_duration_us_max: CanonicalU64::new(*durations.last().unwrap_or(&0)),
            latest,
            rolling_slowest,
            rolling_slowest_age_ms: CanonicalU64::new(
                now_ns
                    .saturating_sub(rolling_slowest_ns)
                    .saturating_div(1_000_000),
            ),
            slowest,
            slowest_age_ms: CanonicalU64::new(
                now_ns.saturating_sub(slowest_ns).saturating_div(1_000_000),
            ),
        })
    }

    fn snapshot(
        &self,
        now_ns: u64,
        input: InputTelemetry,
        transport: TransportTelemetry,
    ) -> ProductDevTelemetrySnapshot {
        let progress_age_ms = self
            .progress_samples_ns
            .back()
            .map(|sample| now_ns.saturating_sub(*sample).saturating_div(1_000_000));
        let progress_rate_millihertz = match (
            self.progress_samples_ns.front(),
            self.progress_samples_ns.back(),
        ) {
            (Some(first), Some(last)) if last > first => Some(CanonicalU64::new(
                ((self.progress_samples_ns.len().saturating_sub(1) as u128)
                    .saturating_mul(1_000_000_000_000)
                    .saturating_div(u128::from(last - first)))
                .min(u128::from(u64::MAX)) as u64,
            )),
            _ => None,
        };
        ProductDevTelemetrySnapshot {
            in_flight_operation: self.in_flight_operation,
            in_flight_age_ms: self.in_flight_started_ns.map(|started| {
                CanonicalU64::new(now_ns.saturating_sub(started).saturating_div(1_000_000))
            }),
            last_product_admission_latency_ms: self
                .last_product_admission_latency_ms
                .map(CanonicalU64::new),
            last_input_admission_latency_ms: self
                .last_input_admission_latency_ms
                .map(CanonicalU64::new),
            queued_input_batches: input.batches,
            queued_input_events: input.events,
            input_batch_capacity: MAX_HOST_INPUT_BATCHES,
            oldest_input_age_ms: input.oldest_ns.map(|oldest| {
                CanonicalU64::new(now_ns.saturating_sub(oldest).saturating_div(1_000_000))
            }),
            input_overflow_pending: input.overflowed,
            runtime_progress_rate_millihertz: progress_rate_millihertz,
            runtime_progress_age_ms: progress_age_ms.map(CanonicalU64::new),
            connections: transport.connections,
            subscribers: transport.subscribers,
            output_queue_items: transport.output_queue_items,
            output_queue_capacity: MAX_OUTPUT_QUEUE_ITEMS,
            output_queue_floor: CanonicalU64::new(transport.output_queue_floor),
            output_binding_active: transport.output_binding_active,
            update_attribution: self.update_attribution_snapshot(now_ns),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct InputTelemetry {
    batches: usize,
    events: usize,
    oldest_ns: Option<u64>,
    overflowed: bool,
}

#[derive(Clone, Copy, Default)]
struct TransportTelemetry {
    connections: usize,
    subscribers: usize,
    output_queue_items: usize,
    output_queue_floor: u64,
    output_binding_active: bool,
}

#[derive(Default)]
struct SchedulerWake {
    state: Mutex<bool>,
    signal: Condvar,
}

/// Generation-based output notification shared by every SSE subscriber.
///
/// A generation, rather than a consumed boolean, lets `notify_all` release all
/// current subscribers without one waiter stealing another waiter's signal.
/// Each subscriber samples the generation before reading the retained bus, so
/// publication cannot race between that read and the following sleep.
#[derive(Default)]
struct OutputWake {
    generation: Mutex<u64>,
    signal: Condvar,
}

impl OutputWake {
    fn generation(&self) -> u64 {
        self.generation
            .lock()
            .map(|generation| *generation)
            .unwrap_or(0)
    }

    fn notify(&self) {
        if let Ok(mut generation) = self.generation.lock() {
            *generation = generation.wrapping_add(1);
            self.signal.notify_all();
        }
    }

    fn wait_timeout(&self, observed_generation: u64, timeout: Duration) {
        let Ok(generation) = self.generation.lock() else {
            return;
        };
        let _ = self
            .signal
            .wait_timeout_while(generation, timeout, |generation| {
                *generation == observed_generation
            });
    }
}

impl SchedulerWake {
    fn notify(&self) {
        if let Ok(mut pending) = self.state.lock() {
            *pending = true;
            self.signal.notify_all();
        }
    }

    fn wait_timeout(&self, timeout: Duration) {
        let Ok(mut pending) = self.state.lock() else {
            return;
        };
        if *pending {
            *pending = false;
            return;
        }
        let Ok((mut pending, _)) = self.signal.wait_timeout(pending, timeout) else {
            return;
        };
        *pending = false;
    }
}

struct HostInputMailbox {
    state: Mutex<HostInputMailboxState>,
}

#[derive(Default)]
struct HostInputMailboxState {
    batches: VecDeque<ProductDevInputBatch>,
    queued_events: usize,
    oldest_enqueued_ns: Option<u64>,
    last_drained_oldest_ns: Option<u64>,
    overflowed: bool,
}

impl Default for HostInputMailbox {
    fn default() -> Self {
        Self {
            state: Mutex::new(HostInputMailboxState::default()),
        }
    }
}

impl HostInputMailbox {
    fn enqueue(&self, batch: ProductDevInputBatch, enqueued_ns: Option<u64>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.batches.len() >= MAX_HOST_INPUT_BATCHES {
            // Drop the retained transport prefix and leave an explicit
            // recovery marker. The scheduler will advance the runtime control
            // fence before its next update, so the browser receives a fresh
            // binding instead of a terminal host closure or a hidden gap.
            state.batches.clear();
            state.queued_events = 0;
            state.oldest_enqueued_ns = None;
            state.overflowed = true;
            return false;
        }
        state.queued_events = state.queued_events.saturating_add(batch.events().len());
        if state.oldest_enqueued_ns.is_none() {
            state.oldest_enqueued_ns = enqueued_ns;
        }
        state.batches.push_back(batch);
        true
    }

    fn drain(&self) -> (Vec<ProductDevInputBatch>, bool) {
        let Ok(mut state) = self.state.lock() else {
            return (Vec::new(), false);
        };
        let batches = state.batches.drain(..).collect();
        state.last_drained_oldest_ns = state.oldest_enqueued_ns.take();
        state.queued_events = 0;
        let overflowed = std::mem::take(&mut state.overflowed);
        (batches, overflowed)
    }

    fn telemetry(&self) -> InputTelemetry {
        self.state
            .lock()
            .map(|state| InputTelemetry {
                batches: state.batches.len(),
                events: state.queued_events,
                oldest_ns: state.oldest_enqueued_ns,
                overflowed: state.overflowed,
            })
            .unwrap_or_default()
    }

    fn take_last_drained_oldest_ns(&self) -> Option<u64> {
        self.state
            .lock()
            .ok()
            .and_then(|mut state| state.last_drained_oldest_ns.take())
    }

    fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.batches.clear();
            state.queued_events = 0;
            state.oldest_enqueued_ns = None;
            state.last_drained_oldest_ns = None;
            state.overflowed = false;
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.state
            .lock()
            .map(|state| state.batches.len())
            .unwrap_or_default()
    }
}

fn scheduler_loop<R: ProductDevRuntime>(state: Arc<HostState<R>>, wake: Arc<SchedulerWake>) {
    let clock = Instant::now();
    let mut next_tick = clock;
    loop {
        if state.shutdown.load(Ordering::Acquire) {
            break;
        }
        let schedule_state = match state.runtime.realtime_schedule_state() {
            Ok(value) => value,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductDevLogSeverity::Error,
                    ProductDevLogDisposition::Terminal,
                    error.code(),
                    error.diagnostic(),
                    [],
                );
                break;
            }
        };
        match schedule_state {
            crate::ProductDevRuntimeScheduleState::Unsupported => {
                wake.wait_timeout(SCHEDULER_IDLE_WAIT);
                continue;
            }
            crate::ProductDevRuntimeScheduleState::Shutdown => break,
            crate::ProductDevRuntimeScheduleState::Created
            | crate::ProductDevRuntimeScheduleState::Paused
            | crate::ProductDevRuntimeScheduleState::Faulted => {
                // Resume from a lifecycle transition at the next admitted
                // observation; this reset is only a phase marker, never a
                // substitute for the runtime's fixed-step interval.
                next_tick = Instant::now();
                wake.wait_timeout(SCHEDULER_IDLE_WAIT);
                continue;
            }
            crate::ProductDevRuntimeScheduleState::Running => {}
        }

        let Some(interval) = (match state.runtime.realtime_schedule_interval() {
            Ok(interval) => interval,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductDevLogSeverity::Error,
                    ProductDevLogDisposition::Terminal,
                    error.code(),
                    error.diagnostic(),
                    [],
                );
                break;
            }
        }) else {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductDevLogSeverity::Error,
                ProductDevLogDisposition::Terminal,
                "DEV_HOST_SCHEDULER_CONFIGURATION",
                "realtime runtime did not provide an admitted observation interval",
                [],
            );
            break;
        };
        let fixed_interval = interval.max(Duration::from_nanos(1));

        let now = Instant::now();
        if now < next_tick {
            wake.wait_timeout(next_tick.saturating_duration_since(now));
            continue;
        }
        let observed = clock.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let input_errors = state.runtime.advance_realtime_with_input_and_publish(
            || state.input_mailbox.drain(),
            CanonicalU64::new(observed),
            |receipt| publish_scheduled_input_receipt(&state, receipt),
            |receipt| publish_scheduled_receipt(&state, receipt),
            || {
                begin_telemetry(&state, ProductDevOperationKind::AdvanceRealtime);
            },
            || {
                let finished_ns = state.diagnostics.now_monotonic_nanoseconds();
                let oldest_ns = state.input_mailbox.take_last_drained_oldest_ns();
                if let Ok(mut telemetry) = state.telemetry.lock() {
                    if let Some(finished_ns) = finished_ns {
                        telemetry.finish(finished_ns);
                        if let Some(oldest_ns) = oldest_ns {
                            telemetry.record_input_admission(
                                finished_ns
                                    .saturating_sub(oldest_ns)
                                    .saturating_div(1_000_000),
                            );
                        }
                    }
                }
            },
        );
        match input_errors {
            Ok((errors, attribution)) => {
                record_update_attribution(&state, attribution);
                for error in errors {
                    publish_host_diagnostic(
                        &state.diagnostics,
                        ProductDevLogSeverity::Warning,
                        disposition_for_runtime_error(&error),
                        error.code(),
                        error.diagnostic(),
                        [],
                    );
                    request_incarnation_replacement(&state, &error);
                }
            }
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductDevLogSeverity::Warning,
                    disposition_for_runtime_error(&error),
                    error.code(),
                    error.diagnostic(),
                    [],
                );
                request_incarnation_replacement(&state, &error);
            }
        }
        let after = Instant::now();
        next_tick = next_tick
            .checked_add(fixed_interval)
            .filter(|deadline| *deadline > after)
            .unwrap_or_else(|| after + fixed_interval);
    }
}

fn request_incarnation_replacement<R: ProductDevRuntime>(
    state: &HostState<R>,
    error: &ProductDevRuntimeError,
) {
    if error.recovery().next_action() != ProductDevNextAction::ReplaceIncarnation {
        return;
    }
    if state.disposable_worker_runtime {
        return;
    }
    if state.shutdown.swap(true, Ordering::SeqCst) {
        return;
    }
    publish_host_diagnostic(
        &state.diagnostics,
        ProductDevLogSeverity::Error,
        ProductDevLogDisposition::Terminal,
        "DEV_HOST_REPLACE_INCARNATION",
        "runtime recovery requires a fresh product-host process incarnation",
        [("cause", error.code().to_owned())],
    );
    state.scheduler_wake.notify();
    state.output_wake.notify();
}

fn disposition_for_runtime_error(error: &ProductDevRuntimeError) -> ProductDevLogDisposition {
    match crate::runtime_fault_disposition(error) {
        crate::ProductDevFaultDisposition::Accepted => ProductDevLogDisposition::Accepted,
        crate::ProductDevFaultDisposition::RejectedRecoverable => {
            ProductDevLogDisposition::RejectedRecoverable
        }
        crate::ProductDevFaultDisposition::Degraded => ProductDevLogDisposition::Degraded,
        crate::ProductDevFaultDisposition::ResyncRequired => {
            ProductDevLogDisposition::ResyncRequired
        }
        crate::ProductDevFaultDisposition::Terminal => ProductDevLogDisposition::Terminal,
    }
}

fn publish_scheduled_input_receipt<R: ProductDevRuntime>(
    state: &HostState<R>,
    receipt: crate::ProductDevRuntimeReceipt<ProductDevInputResult>,
) {
    let (result, mut outputs) = receipt.into_parts();
    // Input is admitted by the Rust-host scheduler rather than by the HTTP
    // request thread. Carry the complete typed result through the same
    // ordered SSE output family so accepted/consumed cursors and recoverable
    // stale drops remain observable without delaying POST acknowledgement.
    outputs.insert(0, ProductDevRuntimeOutput::runtime_input_result(result));
    // Before a browser has attached there is no active output binding to
    // publish against. A later fresh SSE connection receives its own complete
    // baseline, so dropping this pre-attachment receipt is safe.
    let active = state
        .outputs
        .lock()
        .map(|outputs| outputs.active_binding.is_some())
        .unwrap_or(false);
    if !active {
        return;
    }
    if let Err(error) = push_host_outputs(state, outputs) {
        publish_host_diagnostic(
            &state.diagnostics,
            ProductDevLogSeverity::Warning,
            ProductDevLogDisposition::ResyncRequired,
            "DEV_HOST_SCHEDULE_INPUT_OUTPUT_RESYNC",
            error.detail(),
            [("cause", error.code().to_owned())],
        );
    }
}

fn publish_scheduled_receipt<R: ProductDevRuntime>(
    state: &HostState<R>,
    receipt: crate::ProductDevRuntimeReceipt<ProductDevOperationResult>,
) {
    let progress_ns = state.diagnostics.now_monotonic_nanoseconds();
    if let Some(progress_ns) = progress_ns {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            telemetry.record_progress(progress_ns);
        }
    }
    let (result, mut outputs) = receipt.into_parts();
    if let Some(readout) = result.readout().cloned() {
        outputs.push(ProductDevRuntimeOutput::runtime_readout(readout));
    }
    outputs.push(ProductDevRuntimeOutput::runtime_progress());
    // Before a browser has attached there is no active output binding to
    // publish against. A later fresh SSE connection receives its own complete
    // baseline, so dropping these pre-attachment progress pulses is safe.
    let active = state
        .outputs
        .lock()
        .map(|outputs| outputs.active_binding.is_some())
        .unwrap_or(false);
    if !active {
        return;
    }
    if let Err(error) = push_host_outputs(state, outputs) {
        publish_host_diagnostic(
            &state.diagnostics,
            ProductDevLogSeverity::Warning,
            ProductDevLogDisposition::ResyncRequired,
            "DEV_HOST_SCHEDULE_OUTPUT_RESYNC",
            error.detail(),
            [("cause", error.code().to_owned())],
        );
    }
}

fn accept_loop<R: ProductDevRuntime>(
    listener: TcpListener,
    state: Arc<HostState<R>>,
    handler_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    accept_decision_hook: Option<AcceptDecisionHook>,
) {
    let mut retry_backoff = ACCEPT_RETRY_INITIAL_BACKOFF;
    while !state.shutdown.load(Ordering::Acquire) {
        reap_finished_handlers(&handler_threads);
        match accept_once(&listener, accept_decision_hook.as_deref()) {
            Ok((stream, _)) => {
                retry_backoff = ACCEPT_RETRY_INITIAL_BACKOFF;
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
            Err(error) => match classify_accept_error(&error) {
                AcceptErrorDisposition::Retry => {
                    publish_host_diagnostic(
                        &state.diagnostics,
                        ProductDevLogSeverity::Warning,
                        ProductDevLogDisposition::RejectedRecoverable,
                        "DEV_HOST_LISTENER_ACCEPT_RETRY",
                        "listener accept failed transiently; retaining listener ownership and retrying",
                        [("backoff-ms", retry_backoff.as_millis().to_string())],
                    );
                    thread::sleep(retry_backoff);
                    retry_backoff = retry_backoff
                        .saturating_mul(2)
                        .min(ACCEPT_RETRY_MAX_BACKOFF);
                }
                AcceptErrorDisposition::Terminal => {
                    publish_host_diagnostic(
                        &state.diagnostics,
                        ProductDevLogSeverity::Error,
                        ProductDevLogDisposition::Terminal,
                        "DEV_HOST_LISTENER_ACCEPT_TERMINAL",
                        "listener accept failed with an irrecoverable listener or ownership state",
                        [("error-kind", format!("{:?}", error.kind()))],
                    );
                    break;
                }
            },
        }
    }
}

fn accept_once(
    listener: &TcpListener,
    test_hook: Option<&(dyn Fn() -> Option<io::ErrorKind> + Send + Sync)>,
) -> io::Result<(TcpStream, SocketAddr)> {
    if let Some(kind) = test_hook.and_then(|hook| hook()) {
        return Err(io::Error::new(kind, "injected listener accept failure"));
    }
    listener.accept()
}

#[derive(Clone, Copy)]
enum AcceptErrorDisposition {
    Retry,
    Terminal,
}

fn classify_accept_error(error: &io::Error) -> AcceptErrorDisposition {
    match error.kind() {
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
            AcceptErrorDisposition::Retry
        }
        _ => AcceptErrorDisposition::Terminal,
    }
}

fn publish_host_diagnostic<const N: usize>(
    diagnostics: &ProductDevLog,
    severity: ProductDevLogSeverity,
    disposition: ProductDevLogDisposition,
    code: &str,
    message: &str,
    fields: [(&str, String); N],
) {
    let Ok(mut event) = ProductDevLogEvent::new(severity, disposition, "dev-host", code, message)
    else {
        return;
    };
    for (key, value) in fields {
        let Ok(next) = event.with_field(key, value) else {
            return;
        };
        event = next;
    }
    let _ = diagnostics.publish(event);
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
    if request.method == "GET"
        && matches!(
            request.path.as_str(),
            "/__rusty/product/runtime/outputs" | "/__rusty/product/runtime/outputs/fresh"
        )
    {
        let fresh = request.path.ends_with("/fresh");
        handle_sse(stream, state, request, fresh);
        return;
    }
    let response = dispatch_request(&state, request);
    let commit = response.commit_disposition;
    if let Err(error) = write_response(&mut stream, response) {
        if commit.is_some() {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductDevLogSeverity::Warning,
                ProductDevLogDisposition::ResyncRequired,
                "DEV_HOST_RESPONSE_WRITE_RESYNC",
                "response delivery failed after an authoritative runtime receipt; reconnect for a fresh readout instead of replaying",
                [("error-kind", format!("{:?}", error.kind()))],
            );
        }
    }
}

fn dispatch_request<R: ProductDevRuntime>(
    state: &HostState<R>,
    request: HttpRequest,
) -> HttpResponse {
    let _projection = state.projection_gate.read().ok();
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
            if let Ok(bundle) = state.bundle.read() {
                if let Some(entry) = bundle.get(&request.path) {
                    return HttpResponse::bytes(200, entry.content_type(), entry.bytes().to_vec());
                }
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
        "/__rusty/product/runtime/ghost-plate-feedback" => {
            invoke_ghost_plate_feedback(state, &request.body)
        }
        "/__rusty/product/runtime/renderer-diagnostics" => {
            invoke_renderer_diagnostics(state, &request.body)
        }
        "/__rusty/product/runtime/diagnostics/read" => {
            invoke_diagnostics_read(state, &request.body)
        }
        "/__rusty/product/runtime/browser-diagnostics" => {
            invoke_browser_diagnostics(state, &request.body)
        }
        _ => HttpResponse::error(404, "DEV_HOST_ROUTE_NOT_FOUND", "route is not admitted"),
    }
}

fn invoke_debug_catalog<R: ProductDevRuntime>(state: &HostState<R>) -> HttpResponse {
    match state.runtime.with_locked_runtime_timed(
        || begin_telemetry(state, ProductDevOperationKind::ExecuteDebug),
        |runtime| {
            let receipt = match runtime.describe_debug() {
                Ok(receipt) => receipt,
                Err(error) => {
                    return Ok(HttpResponse::error(500, error.code(), error.diagnostic()))
                }
            };
            let (catalog, outputs) = receipt.into_parts();
            let output_through = match push_host_outputs(state, outputs) {
                Ok(output_through) => output_through,
                Err(error) => return Ok(HttpResponse::error(503, error.code(), error.detail())),
            };
            Ok(json_response(200, &catalog).with_output_through(output_through))
        },
        || finish_telemetry(state, ProductDevOperationKind::ExecuteDebug),
    ) {
        Ok(response) => response,
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
}

fn invoke_debug_execute<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    if body.len() > crate::ProductDevDebugResult::MAX_MESSAGE_BYTES {
        return debug_text_error(413, "debug command exceeds host bound");
    }
    let command = match std::str::from_utf8(body) {
        Ok(command) => command,
        Err(_) => return debug_text_error(400, "debug command body must be valid UTF-8"),
    };
    match state.runtime.with_locked_runtime_timed(
        || begin_telemetry(state, ProductDevOperationKind::ExecuteDebug),
        |runtime| {
            let receipt = match runtime.execute_debug(command) {
                Ok(receipt) => receipt,
                Err(error) => {
                    return Ok(debug_text_error(
                        500,
                        &format!("{}: {}", error.code(), error.diagnostic()),
                    ));
                }
            };
            let (result, outputs) = receipt.into_parts();
            let output_through = match push_host_outputs(state, outputs) {
                Ok(output_through) => output_through,
                Err(error) => {
                    return Ok(debug_text_error(
                        503,
                        &format!("{}: {}", error.code(), error.detail()),
                    ));
                }
            };
            Ok(HttpResponse::text(
                if result.succeeded() { 200 } else { 422 },
                result.message().to_owned(),
            )
            .with_output_through(output_through))
        },
        || finish_telemetry(state, ProductDevOperationKind::ExecuteDebug),
    ) {
        Ok(response) => response,
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
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
    // A lifecycle transition changes the input binding or terminal state.
    // Discard transport work queued before that fence before taking the owner
    // operation; a concurrent scheduler still serializes its already-drained
    // prefix through the same runtime lock.
    state.input_mailbox.clear();
    let response = call_runtime(
        state,
        operation.operation_kind(),
        |runtime| runtime.lifecycle_with_binding(operation, request.runtime),
        |error| ProductDevOperationResult::rejected_runtime(operation.operation_kind(), error),
    );
    state.scheduler_wake.notify();
    response
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
    state.input_mailbox.clear();
    let response = call_runtime(
        state,
        operation.operation_kind(),
        |runtime| runtime.control(operation, request.runtime),
        |error| ProductDevOperationResult::rejected_runtime(operation.operation_kind(), error),
    );
    state.scheduler_wake.notify();
    response
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
            );
        }
    };
    let batch = match ProductDevInputBatch::decode_json(&batch_json) {
        Ok(batch) => batch,
        Err(_) => return recover_rejected_input_batch(state, request.batch.len()),
    };
    // Realtime products enqueue at the host edge so a product callback cannot
    // hold up browser input transport. The cached capability covers
    // Created/Paused states as well; demand/external runtimes retain their
    // direct receipt semantics and remain caller-driven.
    if state.realtime_scheduler_enabled {
        let count = batch.events().len();
        let enqueued_ns = state.diagnostics.now_monotonic_nanoseconds();
        return match state.input_mailbox.enqueue(batch, enqueued_ns) {
            true => {
                state.scheduler_wake.notify();
                match ProductDevInputResult::queued(count) {
                    Ok(result) => json_response(200, &result),
                    Err(error) => HttpResponse::error(500, error.code(), error.detail()),
                }
            }
            false => {
                state.scheduler_wake.notify();
                match ProductDevInputResult::mailbox_full(count) {
                    Ok(result) => json_response(200, &result).with_resync_required(),
                    Err(error) => HttpResponse::error(500, error.code(), error.detail()),
                }
            }
        };
    }
    call_runtime(
        state,
        ProductDevOperationKind::Input,
        |runtime| runtime.input(batch),
        |error| crate::ProductDevInputResult::rejected_runtime(error),
    )
}

fn recover_rejected_input_batch<R: ProductDevRuntime>(
    state: &HostState<R>,
    count: usize,
) -> HttpResponse {
    state.input_mailbox.clear();
    publish_host_diagnostic(
        &state.diagnostics,
        ProductDevLogSeverity::Warning,
        ProductDevLogDisposition::ResyncRequired,
        "DEV_HOST_INPUT_DECODE",
        "input batch was not a strict runtime-input wire batch; replacing the runtime input binding before continuing",
        [("submitted-count", count.to_string())],
    );
    let response = call_runtime(
        state,
        ProductDevOperationKind::Input,
        |runtime| {
            let recovery = runtime.recover_input_overflow()?;
            let (_operation, outputs) = recovery.into_parts();
            let result = ProductDevInputResult::wire_decode_resynchronized(count)
                .map_err(host_error_to_runtime)?;
            ProductDevRuntimeReceipt::new(result, outputs).map_err(host_error_to_runtime)
        },
        ProductDevInputResult::rejected_runtime,
    );
    state.scheduler_wake.notify();
    response
}

fn host_error_to_runtime(error: ProductDevHostError) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(error.code(), error.detail())
        .expect("bounded host error has a valid runtime diagnostic")
}

fn invoke_realtime<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: RealtimeRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        ProductDevOperationKind::AdvanceRealtime,
        |runtime| runtime.advance_realtime(request.observed_time_ns),
        |error| {
            ProductDevOperationResult::rejected_runtime(
                ProductDevOperationKind::AdvanceRealtime,
                error,
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
        ProductDevOperationKind::AdmitDemandStep,
        |runtime| runtime.admit_demand_step(),
        |error| {
            ProductDevOperationResult::rejected_runtime(
                ProductDevOperationKind::AdmitDemandStep,
                error,
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
        ProductDevOperationKind::AdmitExternalStep,
        |runtime| runtime.admit_external_step(request.step),
        |error| {
            ProductDevOperationResult::rejected_runtime(
                ProductDevOperationKind::AdmitExternalStep,
                error,
            )
        },
    )
}

fn invoke_timeline<R: ProductDevRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request = match ProductDevTimelineCompletion::decode_json(body) {
        Ok(value) => value,
        Err(error) => return HttpResponse::error(400, error.code(), error.detail()),
    };
    let ticket = request.envelope().ticket().value();
    call_runtime(
        state,
        ProductDevOperationKind::CompleteTimeline,
        |runtime| runtime.complete_timeline(request),
        |error| {
            crate::ProductDevTimelineCompletionResult::rejected_runtime(
                CanonicalU64::new(ticket),
                error,
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
        ProductDevOperationKind::ReportAudioFeedback,
        |runtime| runtime.report_audio_feedback(request),
        |error| crate::ProductDevAudioFeedbackResult::rejected_runtime(binding, error),
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
        ProductDevOperationKind::ReportAnimationFeedback,
        |runtime| runtime.report_animation_feedback(request),
        |error| crate::ProductDevAnimationFeedbackResult::rejected_runtime(binding, error),
    )
}

fn invoke_ghost_plate_feedback<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    let request: crate::ProductDevGhostPlateFeedback = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = request.validate() {
        return HttpResponse::error(400, error.code(), error.detail());
    }
    let binding = request.runtime;
    call_runtime(
        state,
        ProductDevOperationKind::ReportGhostPlateFeedback,
        |runtime| runtime.report_ghost_plate_feedback(request),
        |error| crate::ProductDevGhostPlateFeedbackResult::rejected_runtime(binding, error),
    )
}

fn invoke_renderer_diagnostics<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    let request: crate::ProductDevRendererDiagnosticsFeedback = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let binding = request.runtime;
    call_runtime(
        state,
        ProductDevOperationKind::ReportRendererDiagnostics,
        |runtime| runtime.report_renderer_diagnostics(request),
        |error| {
            crate::ProductDevRendererDiagnosticsFeedbackResult::rejected_runtime(binding, error)
        },
    )
}

fn invoke_diagnostics_read<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    if !state.live_debug_enabled {
        return HttpResponse::error(404, "DEV_HOST_ROUTE_NOT_FOUND", "route is not admitted");
    }
    let request: DiagnosticsReadRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let batch = state
        .diagnostics
        .read_after(request.after.map(CanonicalU64::get));
    let telemetry = telemetry_snapshot(state, batch.read_monotonic_nanoseconds);
    json_response(200, &DiagnosticsReadResponse { batch, telemetry })
}

fn telemetry_snapshot<R: ProductDevRuntime>(
    state: &HostState<R>,
    now_ns: u64,
) -> ProductDevTelemetrySnapshot {
    let input = state.input_mailbox.telemetry();
    let transport = state
        .outputs
        .lock()
        .map(|outputs| TransportTelemetry {
            connections: state.connections.load(Ordering::Acquire),
            subscribers: state.subscribers.load(Ordering::Acquire),
            output_queue_items: outputs.events.len(),
            output_queue_floor: outputs.floor_cursor,
            output_binding_active: outputs.active_binding.is_some(),
        })
        .unwrap_or_else(|_| TransportTelemetry {
            connections: state.connections.load(Ordering::Acquire),
            subscribers: state.subscribers.load(Ordering::Acquire),
            ..TransportTelemetry::default()
        });
    state
        .telemetry
        .lock()
        .map(|telemetry| telemetry.snapshot(now_ns, input, transport))
        .unwrap_or_else(|_| HostTelemetry::default().snapshot(now_ns, input, transport))
}

fn invoke_browser_diagnostics<R: ProductDevRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    let report: ProductDevBrowserDiagnosticsReport = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = report.validate() {
        return HttpResponse::error(400, error.code(), error.detail());
    }
    let mut reported = 0_u8;
    let status_disposition = if matches!(
        report.host_state,
        ProductDevBrowserHostState::Degraded | ProductDevBrowserHostState::Failed
    ) {
        ProductDevLogDisposition::Degraded
    } else {
        ProductDevLogDisposition::Accepted
    };
    let status_severity = if matches!(
        report.host_state,
        ProductDevBrowserHostState::Degraded | ProductDevBrowserHostState::Failed
    ) {
        ProductDevLogSeverity::Warning
    } else {
        ProductDevLogSeverity::Info
    };
    let mut status = match ProductDevLogEvent::new(
        status_severity,
        status_disposition,
        "browser-host",
        "BROWSER_HOST_STATUS",
        "Product Browser Host transition snapshot",
    ) {
        Ok(event) => event,
        Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
    };
    for (key, value) in [
        ("scope", "transition".to_owned()),
        (
            "host-state",
            browser_host_state(report.host_state).to_owned(),
        ),
        (
            "runtime-progress",
            report.runtime_progress.get().to_string(),
        ),
        (
            "transport",
            browser_connection_state(report.transport_state).to_owned(),
        ),
        (
            "output",
            browser_connection_state(report.output_state).to_owned(),
        ),
        (
            "renderer-sequence",
            report
                .last_renderer_sequence
                .map_or_else(|| "none".to_owned(), |value| value.get().to_string()),
        ),
        (
            "renderer-observation-age-ms",
            report
                .renderer_observation_age_ms
                .map_or_else(|| "none".to_owned(), |value| value.get().to_string()),
        ),
    ] {
        status = match status.with_field(key, value) {
            Ok(event) => event,
            Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
        };
    }
    if let Err(error) = state.diagnostics.publish(status) {
        return HttpResponse::error(503, error.code(), error.detail());
    }
    reported = reported.saturating_add(1);

    if let Some(terminal) = report.first_terminal {
        let event = match ProductDevLogEvent::new(
            ProductDevLogSeverity::Error,
            ProductDevLogDisposition::Terminal,
            "browser-host",
            terminal.code,
            terminal.message,
        ) {
            Ok(event) => event,
            Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
        };
        if let Err(error) = state.diagnostics.publish(event) {
            return HttpResponse::error(503, error.code(), error.detail());
        }
        reported = reported.saturating_add(1);
    }
    if let Some(recoverable) = report.recoverable_event {
        let event = match ProductDevLogEvent::new(
            ProductDevLogSeverity::Warning,
            ProductDevLogDisposition::RejectedRecoverable,
            "browser-host",
            recoverable.code,
            recoverable.message,
        ) {
            Ok(event) => event,
            Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
        };
        if let Err(error) = state.diagnostics.publish(event) {
            return HttpResponse::error(503, error.code(), error.detail());
        }
        reported = reported.saturating_add(1);
    }
    for page_event in report.page_events {
        let event = match ProductDevLogEvent::new(
            ProductDevLogSeverity::Warning,
            ProductDevLogDisposition::Degraded,
            "browser-page",
            page_event.code,
            page_event.message,
        )
        .and_then(|event| event.with_field("kind", browser_page_diagnostic_kind(page_event.kind)))
        {
            Ok(event) => event,
            Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
        };
        if let Err(error) = state.diagnostics.publish(event) {
            return HttpResponse::error(503, error.code(), error.detail());
        }
        reported = reported.saturating_add(1);
    }
    json_response(
        200,
        &ProductDevBrowserDiagnosticsResult {
            accepted: true,
            reported,
        },
    )
    // Browser diagnostics append to the host-owned diagnostic log. It has no
    // retained runtime output cursor, but the accepted mutation is still
    // definitive and must use the same browser transport commit boundary.
    .with_committed()
}

fn browser_host_state(state: ProductDevBrowserHostState) -> &'static str {
    match state {
        ProductDevBrowserHostState::Loading => "loading",
        ProductDevBrowserHostState::Ready => "ready",
        ProductDevBrowserHostState::Degraded => "degraded",
        ProductDevBrowserHostState::Failed => "failed",
        ProductDevBrowserHostState::Disposed => "disposed",
    }
}

fn browser_connection_state(state: ProductDevBrowserConnectionState) -> &'static str {
    match state {
        ProductDevBrowserConnectionState::Open => "open",
        ProductDevBrowserConnectionState::Closed => "closed",
    }
}

fn browser_page_diagnostic_kind(kind: crate::ProductDevBrowserPageDiagnosticKind) -> &'static str {
    match kind {
        crate::ProductDevBrowserPageDiagnosticKind::Error => "error",
        crate::ProductDevBrowserPageDiagnosticKind::UnhandledRejection => "unhandled-rejection",
    }
}

fn begin_telemetry<R: ProductDevRuntime>(state: &HostState<R>, operation: ProductDevOperationKind) {
    if let Some(started_ns) = state.diagnostics.now_monotonic_nanoseconds() {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            telemetry.begin(operation, started_ns);
        }
    }
}

fn finish_telemetry<R: ProductDevRuntime>(
    state: &HostState<R>,
    operation: ProductDevOperationKind,
) {
    if let Some(finished_ns) = state.diagnostics.now_monotonic_nanoseconds() {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            if let Some(latency_ms) = telemetry.finish(finished_ns) {
                if matches!(operation, ProductDevOperationKind::Input) {
                    telemetry.record_input_admission(latency_ms);
                }
            }
        }
    }
}

fn record_update_attribution<R: ProductDevRuntime>(
    state: &HostState<R>,
    attribution: Option<ProductDevUpdateAttribution>,
) {
    let Some(attribution) = attribution else {
        return;
    };
    let Some(completed_ns) = state.diagnostics.now_monotonic_nanoseconds() else {
        return;
    };
    if let Ok(mut telemetry) = state.telemetry.lock() {
        telemetry.record_update_attribution(completed_ns, attribution);
    }
}

fn call_runtime<R, T, F, E>(
    state: &HostState<R>,
    operation: ProductDevOperationKind,
    call: F,
    error_result: E,
) -> HttpResponse
where
    R: ProductDevRuntime,
    T: Serialize,
    F: FnOnce(&mut R) -> Result<crate::ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>,
    E: FnOnce(ProductDevRuntimeError) -> Result<T, ProductDevHostError>,
{
    let response = state.runtime.with_locked_runtime_timed(
        || begin_telemetry(state, operation),
        |runtime| {
        let receipt = match call(runtime) {
            Ok(receipt) => receipt,
            Err(error) => {
                let message = if error.diagnostic().is_empty() {
                    "runtime operation failed"
                } else {
                    error.diagnostic()
                };
                let _ = state.diagnostics.publish(
                    crate::ProductDevLogEvent::new(
                        crate::ProductDevLogSeverity::Error,
                        disposition_for_runtime_error(&error),
                        "runtime",
                        error.code(),
                        message,
                    )
                    .expect("runtime diagnostics are bounded"),
                );
                request_incarnation_replacement(state, &error);
                return Ok((match error_result(error) {
                    Ok(result) => json_response(200, &result),
                    Err(host_error) => {
                        HttpResponse::error(500, host_error.code(), host_error.detail())
                    }
                }, runtime.take_update_attribution()));
            }
        };
        let (result, outputs) = receipt.into_parts();
        // Encoding is a host-owned preflight. A bad response must not first
        // publish retained output for a runtime mutation the client cannot name.
        let encoded_result = match encode_runtime_result(&result) {
            Ok(bytes) => bytes,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductDevLogSeverity::Warning,
                    ProductDevLogDisposition::ResyncRequired,
                    "DEV_HOST_RESPONSE_ENCODE_RESYNC",
                    "runtime result could not be encoded after mutation; reconnect for a fresh readout instead of replaying",
                    [("cause", error.code().to_owned())],
                );
                return Ok((HttpResponse::error(500, error.code(), error.detail()), runtime.take_update_attribution()));
            }
        };
        let output_through = match push_host_outputs(state, outputs) {
            Ok(output_through) => output_through,
            Err(error) => {
                // The typed route result names the exact consumed binding,
                // input sequence/readout, or timeline ticket. Preserve it with
                // the closed resync disposition so callers never blindly
                // replay a request after the runtime has already mutated.
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductDevLogSeverity::Warning,
                    ProductDevLogDisposition::ResyncRequired,
                    "DEV_HOST_OUTPUT_COMMIT_RESYNC",
                    "retained output publication failed after an authoritative runtime receipt; reconnect for a fresh readout instead of replaying",
                    [("cause", error.code().to_owned())],
                );
                return Ok((HttpResponse::bytes(200, "application/json", encoded_result)
                    .with_resync_required(), runtime.take_update_attribution()));
            }
        };
        Ok((HttpResponse::bytes(200, "application/json", encoded_result)
            .with_output_through(output_through), runtime.take_update_attribution()))
        },
        || finish_telemetry(state, operation),
    );
    match response {
        Ok((response, attribution)) => {
            record_update_attribution(state, attribution);
            response
        }
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
}

fn encode_runtime_result<T: Serialize>(value: &T) -> Result<Vec<u8>, ProductDevHostError> {
    match serde_json::to_vec(value) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST_BODY_BYTES => Ok(bytes),
        Ok(_) => Err(ProductDevHostError::new(
            "DEV_HOST_RESPONSE_BOUNDS",
            "runtime result exceeds response bound",
        )),
        Err(_) => Err(ProductDevHostError::new(
            "DEV_HOST_RESPONSE_ENCODE",
            "runtime result could not be encoded",
        )),
    }
}

fn handle_sse<R: ProductDevRuntime>(
    mut stream: TcpStream,
    state: Arc<HostState<R>>,
    request: HttpRequest,
    fresh: bool,
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
    let fresh_connection = fresh && !request.headers.contains_key("last-event-id");
    let mut private_events = Vec::new();
    let mut connection_result = None;
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
        None if fresh_connection => {
            let connection = state.runtime.with_locked_runtime_timed(
                || begin_telemetry(&state, ProductDevOperationKind::Connect),
                |runtime| {
                    let receipt = runtime.connect()?;
                    let (result, outputs) = receipt.into_parts();
                    let isolated = Mutex::new(OutputBus::default());
                    if let Err(error) = push_outputs(&isolated, outputs) {
                        return Ok(Err(HttpResponse::error(503, error.code(), error.detail())));
                    }
                    let isolated = match isolated.into_inner() {
                        Ok(bus) => bus,
                        Err(_) => {
                            return Ok(Err(HttpResponse::error(
                                500,
                                "DEV_HOST_OUTPUT_POISONED",
                                "isolated output queue lock is poisoned",
                            )));
                        }
                    };
                    let Some(connection_binding) = isolated.active_binding else {
                        return Ok(Err(HttpResponse::error(
                            503,
                            "DEV_HOST_OUTPUT_BASELINE",
                            "runtime connection did not publish a complete binding baseline",
                        )));
                    };
                    let result_json = match serde_json::to_string(&result) {
                        Ok(result) => result,
                        Err(_) => {
                            return Ok(Err(HttpResponse::error(
                                500,
                                "DEV_HOST_RESPONSE_ENCODE",
                                "runtime connection result could not be encoded",
                            )));
                        }
                    };
                    let mut outputs = match state.outputs.lock() {
                        Ok(outputs) => outputs,
                        Err(_) => {
                            return Ok(Err(HttpResponse::error(
                                500,
                                "DEV_HOST_OUTPUT_POISONED",
                                "output queue lock is poisoned",
                            )));
                        }
                    };
                    // Keep runtime connection, binding publication, and cursor
                    // assignment in one owner section so a scheduler receipt
                    // cannot be emitted between them.
                    outputs.active_binding = Some(connection_binding);
                    private_events = isolated.events.into_iter().collect();
                    connection_result = Some(result_json);
                    let cursor = outputs.next_id;
                    Ok(Ok(cursor))
                },
                || finish_telemetry(&state, ProductDevOperationKind::Connect),
            );
            match connection {
                Ok(Ok(cursor)) => {
                    state.scheduler_wake.notify();
                    cursor
                }
                Ok(Err(response)) => {
                    let _ = write_response(&mut stream, response);
                    return;
                }
                Err(error) => {
                    request_incarnation_replacement(&state, &error);
                    let _ = write_response(
                        &mut stream,
                        HttpResponse::error(500, error.code(), error.diagnostic()),
                    );
                    return;
                }
            }
        }
        None => 0,
    };
    if write_sse_headers(&mut stream).is_err() {
        return;
    }
    for event in private_events {
        if write_sse_private_event(&mut stream, &event).is_err() {
            return;
        }
    }
    if let Some(result) = connection_result {
        // Subscriber-private baselines deliberately carry no SSE cursor. An
        // id parsed before this record's terminating blank line could survive
        // a disconnect even though JavaScript never received the completion.
        let payload = format!("event: rusty-output-baseline\ndata: {result}\n\n");
        if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
            return;
        }
    }
    let mut last_write = Instant::now();
    loop {
        if state.shutdown.load(Ordering::Acquire) {
            break;
        }
        let (observed_generation, snapshot) = {
            let _projection = state.projection_gate.read().ok();
            let observed_generation = state.output_wake.generation();
            let snapshot = match state.outputs.lock() {
                Ok(outputs) => outputs.after(cursor),
                Err(_) => break,
            };
            (observed_generation, snapshot)
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
        let had_events = !snapshot.events.is_empty();
        for event in snapshot.events {
            if write_sse_event(&mut stream, &event).is_err() {
                return;
            }
            cursor = event.id;
            last_write = Instant::now();
        }
        if had_events {
            continue;
        }
        let heartbeat_wait = SSE_HEARTBEAT_INTERVAL.saturating_sub(last_write.elapsed());
        if heartbeat_wait.is_zero() {
            if stream.write_all(b": rusty-keep-alive\n\n").is_err() || stream.flush().is_err() {
                return;
            }
            last_write = Instant::now();
            continue;
        }
        state
            .output_wake
            .wait_timeout(observed_generation, heartbeat_wait);
    }
}

fn write_sse_private_event(stream: &mut TcpStream, event: &OutputEvent) -> io::Result<()> {
    let payload = match event.event {
        Some(name) => format!("event: {}\ndata: {}\n\n", name, event.json),
        None => format!("data: {}\n\n", event.json),
    };
    stream.write_all(payload.as_bytes())?;
    stream.flush()
}

fn write_sse_event(stream: &mut TcpStream, event: &OutputEvent) -> io::Result<()> {
    let payload = match event.event {
        Some(name) => format!(
            "id: {}\nevent: {}\ndata: {}\n\n",
            event.id, name, event.json
        ),
        None => format!("id: {}\ndata: {}\n\n", event.id, event.json),
    };
    stream.write_all(payload.as_bytes())?;
    stream.flush()
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

#[derive(Clone)]
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
    // Encode, fragment, order-check, and baseline-check against a bounded
    // transaction first. A receipt arrives only after its product mutation,
    // so partial retained publication would leave a browser with an ambiguous
    // prefix. Existing retained JSON stays in place until the transaction is
    // known-good; a failed preflight instead fences the old binding and
    // requires a fresh baseline.
    match push_outputs_staged(&mut bus, outputs) {
        Ok(output_through) => Ok(output_through),
        Err(error) => {
            bus.active_binding = None;
            bus.pending_baseline = None;
            Err(error)
        }
    }
}

fn push_host_outputs<R: ProductDevRuntime>(
    state: &HostState<R>,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<u64, ProductDevHostError> {
    let changed = !outputs.is_empty();
    let output_through = push_outputs(&state.outputs, outputs)?;
    if changed {
        state.output_wake.notify();
    }
    Ok(output_through)
}

fn push_outputs_staged(
    bus: &mut OutputBus,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<u64, ProductDevHostError> {
    let mut staged = OutputPushStage::new(bus);
    let mut incremental_outputs = Vec::new();
    for output in outputs {
        if let Some(binding) = output.binding_marker() {
            if !incremental_outputs.is_empty() {
                let active_binding = staged.active_binding.ok_or_else(|| {
                    ProductDevHostError::new(
                        "DEV_HOST_OUTPUT_BASELINE",
                        "incremental output arrived before a complete binding baseline",
                    )
                })?;
                append_staged_output_events(
                    bus,
                    &mut staged,
                    active_binding,
                    std::mem::take(&mut incremental_outputs),
                )?;
            }
            if staged.pending_baseline.is_some() {
                return Err(ProductDevHostError::new(
                    "DEV_HOST_OUTPUT_BASELINE",
                    "a new binding arrived before the previous baseline completed",
                ));
            }
            // A runtime that begins a replacement baseline owns subsequent
            // publication. Fence the previous binding immediately so a
            // rejected replacement cannot label later incrementals with the
            // stale runtime identity.
            staged.active_binding = None;
            staged.pending_baseline = Some(PendingBaseline {
                binding,
                outputs: vec![output],
            });
            continue;
        }
        if let Some(binding) = output.complete_baseline_marker() {
            let pending = {
                let Some(pending) = staged.pending_baseline.take() else {
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
                pending
            };
            // Admission is fail-atomic. Once a complete baseline is rejected,
            // discard its staging buffer so the producer can replay the same
            // full binding-to-completion sequence without a phantom baseline.
            append_staged_output_events(bus, &mut staged, pending.binding, pending.outputs)?;
            staged.active_binding = Some(binding);
            continue;
        }
        if let Some(pending) = &mut staged.pending_baseline {
            pending.outputs.push(output);
            continue;
        }
        if staged.active_binding.is_none() {
            return Err(ProductDevHostError::new(
                "DEV_HOST_OUTPUT_BASELINE",
                "incremental output arrived before a complete binding baseline",
            ));
        }
        incremental_outputs.push(output);
    }
    if !incremental_outputs.is_empty() {
        let binding = staged.active_binding.expect("active binding was checked");
        append_staged_output_events(bus, &mut staged, binding, incremental_outputs)?;
    }
    let output_through = staged.next_id;
    staged.commit(bus);
    Ok(output_through)
}

#[cfg(test)]
fn append_output_events(
    bus: &mut OutputBus,
    binding: crate::ProductDevRuntimeBinding,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<(), ProductDevHostError> {
    let mut staged = OutputPushStage::new(bus);
    append_staged_output_events(bus, &mut staged, binding, outputs)?;
    staged.commit(bus);
    Ok(())
}

struct OutputPushStage {
    next_id: u64,
    next_transfer_id: u64,
    active_binding: Option<crate::ProductDevRuntimeBinding>,
    pending_baseline: Option<PendingBaseline>,
    retained_start: usize,
    floor_cursor: u64,
    new_events: VecDeque<OutputEvent>,
}

impl OutputPushStage {
    fn new(bus: &OutputBus) -> Self {
        Self {
            next_id: bus.next_id,
            next_transfer_id: bus.next_transfer_id,
            active_binding: bus.active_binding,
            pending_baseline: bus.pending_baseline.clone(),
            retained_start: 0,
            floor_cursor: bus.floor_cursor,
            new_events: VecDeque::new(),
        }
    }

    fn push_event(&mut self, bus: &OutputBus, event: OutputEvent) {
        let retained_existing = bus.events.len().saturating_sub(self.retained_start);
        if retained_existing + self.new_events.len() == MAX_OUTPUT_QUEUE_ITEMS {
            if self.retained_start < bus.events.len() {
                self.floor_cursor = bus.events[self.retained_start].id;
                self.retained_start += 1;
            } else if let Some(evicted) = self.new_events.pop_front() {
                self.floor_cursor = evicted.id;
            }
        }
        self.new_events.push_back(event);
    }

    fn commit(self, bus: &mut OutputBus) {
        bus.events.drain(..self.retained_start);
        bus.events.extend(self.new_events);
        bus.next_id = self.next_id;
        bus.next_transfer_id = self.next_transfer_id;
        bus.floor_cursor = self.floor_cursor;
        bus.active_binding = self.active_binding;
        bus.pending_baseline = self.pending_baseline;
    }
}

fn append_staged_output_events(
    bus: &OutputBus,
    staged: &mut OutputPushStage,
    binding: crate::ProductDevRuntimeBinding,
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<(), ProductDevHostError> {
    let mut encoded_events = Vec::new();
    let mut next_transfer_id = staged.next_transfer_id;
    let encoded = serde_json::to_string(&serde_json::json!({
        "kind": "runtime-output-batch",
        "outputs": outputs,
    }))
    .map_err(|error| ProductDevHostError::new("DEV_HOST_OUTPUT_ENCODE", error.to_string()))?;
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
    } else {
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
    let final_id = staged
        .next_id
        .checked_add(encoded_events.len() as u64)
        .ok_or_else(|| {
            ProductDevHostError::new("DEV_HOST_OUTPUT_ID", "output sequence exhausted")
        })?;
    for encoded in encoded_events {
        staged.next_id += 1;
        staged.push_event(
            bus,
            OutputEvent {
                id: staged.next_id,
                event: encoded.event,
                json: encoded.json,
            },
        );
    }
    debug_assert_eq!(staged.next_id, final_id);
    staged.next_transfer_id = next_transfer_id;
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

    #[test]
    fn output_wake_releases_every_current_subscriber() {
        const SUBSCRIBERS: usize = 3;
        let wake = Arc::new(OutputWake::default());
        let ready = Arc::new(std::sync::Barrier::new(SUBSCRIBERS + 1));
        let (elapsed, observations) = std::sync::mpsc::channel();
        let mut waiters = Vec::new();
        for _ in 0..SUBSCRIBERS {
            let wake = Arc::clone(&wake);
            let ready = Arc::clone(&ready);
            let elapsed = elapsed.clone();
            waiters.push(thread::spawn(move || {
                let generation = wake.generation();
                ready.wait();
                let started = Instant::now();
                wake.wait_timeout(generation, Duration::from_secs(2));
                elapsed.send(started.elapsed()).unwrap();
            }));
        }

        ready.wait();
        wake.notify();
        for _ in 0..SUBSCRIBERS {
            assert!(
                observations
                    .recv_timeout(Duration::from_millis(500))
                    .is_ok(),
                "output publication did not wake every current subscriber"
            );
        }
        for waiter in waiters {
            waiter.join().unwrap();
        }
    }

    struct BlockingRealtimeRuntime;

    fn blocking_runtime_error() -> crate::ProductDevRuntimeError {
        crate::ProductDevRuntimeError::new("TEST_RUNTIME", "test runtime operation").unwrap()
    }

    impl crate::ProductDevRuntime for BlockingRealtimeRuntime {
        fn realtime_schedule_state(&self) -> crate::ProductDevRuntimeScheduleState {
            crate::ProductDevRuntimeScheduleState::Running
        }

        fn realtime_schedule_interval(&self) -> Option<Duration> {
            Some(Duration::from_millis(1))
        }

        fn lifecycle(
            &mut self,
            _operation: crate::ProductDevLifecycleOperation,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevOperationResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn input(
            &mut self,
            _batch: crate::ProductDevInputBatch,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevInputResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn advance_realtime(
            &mut self,
            _observed_time_ns: CanonicalU64,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevOperationResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_demand_step(
            &mut self,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevOperationResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_external_step(
            &mut self,
            _step: CanonicalU64,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevOperationResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn complete_timeline(
            &mut self,
            _completion: crate::ProductDevTimelineCompletion,
        ) -> Result<
            crate::ProductDevRuntimeReceipt<crate::ProductDevTimelineCompletionResult>,
            crate::ProductDevRuntimeError,
        > {
            Err(blocking_runtime_error())
        }
    }

    fn binding() -> crate::ProductDevRuntimeBinding {
        crate::ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(2),
        }
    }

    #[test]
    fn input_mailbox_overflow_clears_prefix_and_marks_resync() {
        let mailbox = HostInputMailbox::default();
        for _ in 0..MAX_HOST_INPUT_BATCHES {
            assert!(mailbox.enqueue(ProductDevInputBatch::new(Vec::new()), Some(0)));
        }
        assert_eq!(mailbox.len(), MAX_HOST_INPUT_BATCHES);
        assert!(!mailbox.enqueue(ProductDevInputBatch::new(Vec::new()), Some(0)));

        let (batches, overflowed) = mailbox.drain();
        assert!(
            batches.is_empty(),
            "overflow must not retain a stale prefix"
        );
        assert!(
            overflowed,
            "scheduler must receive an explicit resync marker"
        );

        assert!(mailbox.enqueue(ProductDevInputBatch::new(Vec::new()), Some(0)));
        let (batches, overflowed) = mailbox.drain();
        assert_eq!(batches.len(), 1);
        assert!(!overflowed);
    }

    #[test]
    fn telemetry_snapshot_is_bounded_and_keeps_subsecond_rates() {
        let mut telemetry = HostTelemetry::default();
        telemetry.begin(ProductDevOperationKind::AdvanceRealtime, 1_000_000);
        telemetry.record_progress(1_000_000_000);
        telemetry.record_progress(3_000_000_000);
        let snapshot = telemetry.snapshot(
            4_000_000_000,
            InputTelemetry {
                batches: MAX_HOST_INPUT_BATCHES,
                events: runtime_input::MAX_RUNTIME_INPUT_WIRE_EVENTS,
                oldest_ns: Some(1_500_000_000),
                overflowed: true,
            },
            TransportTelemetry {
                connections: 2,
                subscribers: 1,
                output_queue_items: MAX_OUTPUT_QUEUE_ITEMS,
                output_queue_floor: 8,
                output_binding_active: true,
            },
        );
        assert_eq!(
            snapshot.in_flight_operation,
            Some(ProductDevOperationKind::AdvanceRealtime)
        );
        assert_eq!(snapshot.in_flight_age_ms, Some(CanonicalU64::new(3999)));
        assert_eq!(
            snapshot.runtime_progress_rate_millihertz,
            Some(CanonicalU64::new(500)),
            "one update per two seconds remains distinguishable from zero",
        );
        assert_eq!(
            snapshot.runtime_progress_age_ms,
            Some(CanonicalU64::new(1000))
        );
        assert_eq!(snapshot.oldest_input_age_ms, Some(CanonicalU64::new(2500)));
        assert!(snapshot.input_overflow_pending);
        let wire = serde_json::to_value(&snapshot).expect("telemetry is serializable");
        assert_eq!(wire["outputQueueItems"], MAX_OUTPUT_QUEUE_ITEMS);
        assert_eq!(wire["runtimeProgressRateMillihertz"], "500");
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < 4 * 1024);
    }

    #[test]
    fn update_attribution_retains_a_long_window_and_lifetime_slowest_sample() {
        let mut telemetry = HostTelemetry::default();
        let sample = |duration_us| ProductDevUpdateAttribution {
            callback_duration_us: CanonicalU64::new(duration_us),
            ..ProductDevUpdateAttribution::default()
        };
        telemetry.record_update_attribution(1, sample(9_000));
        for duration_us in 1_u64..=2_049 {
            telemetry.record_update_attribution(duration_us * 1_000_000, sample(duration_us));
        }

        let snapshot = telemetry
            .update_attribution_snapshot(3_000_000_000)
            .expect("completed update samples are retained");
        assert_eq!(snapshot.sample_count, CanonicalU64::new(2_048));
        assert_eq!(snapshot.callback_duration_us_max, CanonicalU64::new(2_049));
        assert_eq!(
            snapshot.rolling_slowest.callback_duration_us,
            CanonicalU64::new(2_049)
        );
        assert_eq!(snapshot.rolling_slowest_age_ms, CanonicalU64::new(951));
        assert_eq!(
            snapshot.slowest.callback_duration_us,
            CanonicalU64::new(9_000)
        );
        assert_eq!(snapshot.slowest_age_ms, CanonicalU64::new(2_999));
    }

    #[test]
    fn scheduled_input_result_preserves_cursor_and_recovery_disposition() {
        let accepted = ProductDevInputResult::with_progress(
            2,
            2,
            0,
            Some(CanonicalU64::new(4)),
            Some(CanonicalU64::new(4)),
            CanonicalU64::new(5),
            binding(),
            crate::ProductDevRuntimeReadout::new(
                binding(),
                crate::ProductDevRuntimeMode::Realtime,
                crate::ProductDevRuntimeState::Running,
            ),
        )
        .unwrap();
        let accepted_wire =
            serde_json::to_value(ProductDevRuntimeOutput::runtime_input_result(accepted)).unwrap();
        assert_eq!(accepted_wire["kind"], "runtime-input-result");
        assert_eq!(accepted_wire["result"]["acceptedThrough"], "4");
        assert_eq!(accepted_wire["result"]["consumedThrough"], "4");
        assert_eq!(accepted_wire["result"]["nextInputSequence"], "5");
        assert_eq!(accepted_wire["result"]["disposition"], "accepted");

        let stale = ProductDevInputResult::with_progress(
            2,
            1,
            1,
            Some(CanonicalU64::new(6)),
            Some(CanonicalU64::new(7)),
            CanonicalU64::new(8),
            binding(),
            crate::ProductDevRuntimeReadout::new(
                binding(),
                crate::ProductDevRuntimeMode::Realtime,
                crate::ProductDevRuntimeState::Running,
            ),
        )
        .unwrap();
        let stale_wire =
            serde_json::to_value(ProductDevRuntimeOutput::runtime_input_result(stale)).unwrap();
        assert_eq!(stale_wire["result"]["accepted"], false);
        assert_eq!(stale_wire["result"]["disposition"], "rejected-recoverable");
        assert_eq!(stale_wire["result"]["acceptedThrough"], "6");
        assert_eq!(stale_wire["result"]["consumedThrough"], "7");

        let overflow = ProductDevInputResult::mailbox_full(2).unwrap();
        let overflow_wire =
            serde_json::to_value(ProductDevRuntimeOutput::runtime_input_result(overflow)).unwrap();
        assert_eq!(overflow_wire["result"]["accepted"], false);
        assert_eq!(overflow_wire["result"]["disposition"], "resync-required");
    }

    #[test]
    fn realtime_input_admission_does_not_wait_for_runtime_owner() {
        let runtime = Arc::new(ProductDevOperationOwner::new(BlockingRealtimeRuntime));
        let state = Arc::new(HostState {
            bundle: Arc::new(RwLock::new(
                ProductDevBundle::new(vec![crate::ProductDevBundleEntry::new(
                    "index.html",
                    "text/html; charset=utf-8",
                    Vec::new(),
                )
                .unwrap()])
                .unwrap(),
            )),
            runtime: Arc::clone(&runtime),
            input_mailbox: Arc::new(HostInputMailbox::default()),
            telemetry: Mutex::new(HostTelemetry::default()),
            realtime_scheduler_enabled: true,
            disposable_worker_runtime: false,
            outputs: Arc::new(Mutex::new(OutputBus::default())),
            output_wake: Arc::new(OutputWake::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            scheduler_wake: Arc::new(SchedulerWake::default()),
            bind_host: Ipv4Addr::LOCALHOST,
            expected_port: 0,
            live_debug_enabled: false,
            diagnostics: ProductDevLog::new(Default::default()).unwrap(),
            projection_gate: Arc::new(RwLock::new(())),
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
        });
        let (held, held_ready) = std::sync::mpsc::channel();
        let (release, release_owner) = std::sync::mpsc::channel();
        let locked_runtime = Arc::clone(&runtime);
        let owner_thread = thread::spawn(move || {
            locked_runtime
                .with_locked_runtime(|_| {
                    held.send(()).expect("owner lock marker");
                    release_owner
                        .recv_timeout(Duration::from_secs(1))
                        .expect("owner lock release");
                    Ok(())
                })
                .expect("owner lock fixture");
        });
        held_ready
            .recv_timeout(Duration::from_secs(1))
            .expect("runtime owner was locked");

        let (response, response_ready) = std::sync::mpsc::channel();
        let input_state = Arc::clone(&state);
        let input_thread = thread::spawn(move || {
            response
                .send(invoke_input(&input_state, br#"{"batch":[]}"#))
                .expect("input response");
        });
        let input_response = response_ready
            .recv_timeout(Duration::from_millis(100))
            .expect("input enqueue must not wait for a slow product update");
        assert_eq!(input_response.status, 200);
        assert_eq!(state.input_mailbox.len(), 1);

        release.send(()).expect("release runtime owner");
        input_thread.join().expect("input worker");
        owner_thread.join().expect("owner worker");
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
    fn one_receipt_encodes_as_one_ordered_output_batch() {
        let mut bus = OutputBus {
            active_binding: Some(binding()),
            ..OutputBus::default()
        };
        push_outputs_staged(
            &mut bus,
            vec![
                crate::model::ProductDevRuntimeOutput::runtime_readout(
                    crate::model::ProductDevRuntimeReadout::new(
                        binding(),
                        crate::model::ProductDevRuntimeMode::Realtime,
                        crate::model::ProductDevRuntimeState::Running,
                    ),
                ),
                crate::model::ProductDevRuntimeOutput::runtime_progress(),
            ],
        )
        .expect("receipt batch publishes");
        assert_eq!(bus.events.len(), 1);
        let value: serde_json::Value = serde_json::from_str(&bus.events[0].json).unwrap();
        assert_eq!(value["kind"], "runtime-output-batch");
        assert_eq!(value["outputs"].as_array().map(Vec::len), Some(2));
        assert_eq!(value["outputs"][0]["kind"], "runtime-readout");
        assert_eq!(value["outputs"][1]["kind"], "runtime-progress");
    }

    fn representative_realtime_receipt(tick: u64) -> Vec<ProductDevRuntimeOutput> {
        let runtime = binding();
        let composition = render_host_contracts::RendererViewComposition {
            schema_version: render_host_contracts::RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION,
            cameras: Vec::new(),
            targets: Vec::new(),
            views: Vec::new(),
            presentations: Vec::new(),
        };
        let ui_runtime = runtime_ui::RuntimeUiRuntimeBinding::new(
            runtime_lifecycle::RuntimeInstanceId::new(runtime.instance_id.get()),
            runtime_lifecycle::RuntimeGeneration::new(runtime.generation.get()),
            runtime_lifecycle::RuntimeControlRevision::new(runtime.control_revision.get()),
        );
        let ui_projection = runtime_ui::RuntimeUiProjectionEnvelope::new(
            ui_runtime,
            tick,
            "product.ui",
            "runtime.tick.v1",
            serde_json::json!({"tick": tick}),
        )
        .expect("representative UI projection");
        vec![
            ProductDevRuntimeOutput::test_frame_value(serde_json::json!({"tick": tick})),
            ProductDevRuntimeOutput::view_composition(&composition)
                .expect("representative view composition"),
            ProductDevRuntimeOutput::ui_projection(&ui_projection)
                .expect("representative UI projection output"),
            ProductDevRuntimeOutput::runtime_readout(
                crate::ProductDevRuntimeReadout::new(
                    runtime,
                    crate::ProductDevRuntimeMode::Realtime,
                    crate::ProductDevRuntimeState::Running,
                )
                .with_counters(tick + 1, 0, 0, 0)
                .with_clock(None, Some(tick + 1)),
            ),
            ProductDevRuntimeOutput::runtime_progress(),
        ]
    }

    #[test]
    fn sixty_hertz_receipts_retain_one_sse_event_and_json_parse_per_receipt() {
        const RECEIPTS: usize = 60;
        const OUTPUTS_PER_RECEIPT: usize = 5;
        let expected_kinds = [
            "frame",
            "view-composition",
            "ui-projection",
            "runtime-readout",
            "runtime-progress",
        ];
        let mut bus = OutputBus {
            active_binding: Some(binding()),
            ..OutputBus::default()
        };
        for tick in 0..RECEIPTS {
            push_outputs_staged(&mut bus, representative_realtime_receipt(tick as u64))
                .expect("representative realtime receipt publishes");
        }

        let mut sse_delivery_callbacks = 0;
        let mut json_parse_calls = 0;
        let mut dispatched_kinds = Vec::with_capacity(RECEIPTS * OUTPUTS_PER_RECEIPT);
        for event in &bus.events {
            sse_delivery_callbacks += 1;
            json_parse_calls += 1;
            let payload: Value =
                serde_json::from_str(&event.json).expect("retained SSE event is valid JSON");
            assert_eq!(payload["kind"], "runtime-output-batch");
            let outputs = payload["outputs"]
                .as_array()
                .expect("retained event contains typed outputs");
            assert_eq!(outputs.len(), OUTPUTS_PER_RECEIPT);
            dispatched_kinds.extend(outputs.iter().map(|output| {
                output["kind"]
                    .as_str()
                    .expect("typed output kind")
                    .to_owned()
            }));
        }

        assert_eq!(sse_delivery_callbacks, RECEIPTS);
        assert_eq!(json_parse_calls, RECEIPTS);
        assert_eq!(dispatched_kinds.len(), RECEIPTS * OUTPUTS_PER_RECEIPT);
        assert_eq!(
            dispatched_kinds,
            expected_kinds
                .iter()
                .map(|kind| (*kind).to_owned())
                .cycle()
                .take(RECEIPTS * OUTPUTS_PER_RECEIPT)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            RECEIPTS * OUTPUTS_PER_RECEIPT,
            300,
            "the old one-callback-per-output stream would deliver about 300 outputs",
        );
        assert!(
            sse_delivery_callbacks < RECEIPTS * OUTPUTS_PER_RECEIPT,
            "one SSE delivery per receipt is materially below one per contained output",
        );
    }

    #[test]
    fn full_queue_publication_shares_retained_payloads_and_reports_throughput() {
        const MEASURED_PUBLISHES: usize = 120;
        let runtime = binding();
        let large_payload = "x".repeat(MAX_OUTPUT_EVENT_BYTES + 1);
        let mut large_probe = OutputBus::default();
        append_output_events(
            &mut large_probe,
            runtime,
            vec![ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"payload": large_payload}),
            )],
        )
        .expect("representative large output fragments");
        let large_event_count = large_probe.events.len();
        assert!(large_event_count > 1);

        let bus = Mutex::new(OutputBus {
            active_binding: Some(runtime),
            ..OutputBus::default()
        });
        for sequence in 0..MAX_OUTPUT_QUEUE_ITEMS - large_event_count {
            push_outputs(
                &bus,
                vec![ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"sequence": sequence}),
                )],
            )
            .expect("representative small output publishes");
        }
        let large_ids = {
            let locked = bus.lock().expect("test bus lock");
            large_probe
                .events
                .iter()
                .map(|event| event.id + locked.next_id)
                .collect::<Vec<_>>()
        };
        push_outputs(
            &bus,
            vec![ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"payload": "x".repeat(MAX_OUTPUT_EVENT_BYTES + 1)}),
            )],
        )
        .expect("large fragmented output fills retained queue");
        let (retained_bytes, retained_payloads, initial_floor, initial_next_id) = {
            let locked = bus.lock().expect("test bus lock");
            assert_eq!(locked.events.len(), MAX_OUTPUT_QUEUE_ITEMS);
            assert_eq!(
                locked
                    .events
                    .iter()
                    .filter(|event| event.event == Some("rusty-output-fragment"))
                    .count(),
                large_event_count
            );
            let retained_bytes = locked
                .events
                .iter()
                .map(|event| event.json.len())
                .sum::<usize>();
            let retained_payloads = large_ids
                .iter()
                .map(|id| {
                    let event = locked
                        .events
                        .iter()
                        .find(|event| event.id == *id)
                        .expect("large event is retained");
                    (*id, event.json.as_ptr(), event.json.len())
                })
                .collect::<Vec<_>>();
            (
                retained_bytes,
                retained_payloads,
                locked.floor_cursor,
                locked.next_id,
            )
        };

        let started = Instant::now();
        for sequence in 0..MEASURED_PUBLISHES {
            push_outputs(
                &bus,
                vec![ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"measuredSequence": sequence}),
                )],
            )
            .expect("measured output publishes");
        }
        let elapsed = started.elapsed();

        let locked = bus.lock().expect("test bus lock");
        assert_eq!(locked.events.len(), MAX_OUTPUT_QUEUE_ITEMS);
        assert_eq!(
            locked.floor_cursor,
            initial_floor + MEASURED_PUBLISHES as u64
        );
        assert_eq!(locked.next_id, initial_next_id + MEASURED_PUBLISHES as u64);
        let reconnect = locked.after(locked.floor_cursor);
        assert_eq!(reconnect.floor_cursor, locked.floor_cursor);
        assert_eq!(reconnect.events.len(), MAX_OUTPUT_QUEUE_ITEMS);
        assert_eq!(reconnect.events.first().map(|event| event.id), Some(121));
        assert_eq!(
            reconnect.events.last().map(|event| event.id),
            Some(locked.next_id)
        );
        for (id, pointer, length) in retained_payloads {
            let event = locked
                .events
                .iter()
                .find(|event| event.id == id)
                .expect("large fragmented payload survives the measurement window");
            assert_eq!(
                event.json.as_ptr(),
                pointer,
                "retained JSON was reallocated"
            );
            assert_eq!(event.json.len(), length);
        }
        eprintln!(
            "output-bus full_queue={} retained_bytes={} large_fragments={} publishes={} elapsed_us={} ns_per_publish={}",
            locked.events.len(),
            retained_bytes,
            large_event_count,
            MEASURED_PUBLISHES,
            elapsed.as_micros(),
            elapsed.as_nanos() / MEASURED_PUBLISHES as u128,
        );
    }

    #[test]
    fn failed_multi_segment_receipt_preserves_history_and_fences_binding() {
        let active_runtime = binding();
        let replacement_runtime = crate::ProductDevRuntimeBinding {
            generation: CanonicalU64::new(2),
            ..active_runtime
        };
        let bus = Mutex::new(OutputBus {
            active_binding: Some(active_runtime),
            ..OutputBus::default()
        });
        push_outputs(
            &bus,
            vec![ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"retained": true}),
            )],
        )
        .expect("initial incremental output publishes");
        let (retained_id, retained_pointer, retained_length, next_id, floor_cursor) = {
            let locked = bus.lock().expect("test bus lock");
            let retained = locked.events.front().expect("retained event");
            (
                retained.id,
                retained.json.as_ptr(),
                retained.json.len(),
                locked.next_id,
                locked.floor_cursor,
            )
        };

        let error = push_outputs(
            &bus,
            vec![
                ProductDevRuntimeOutput::runtime_progress(),
                ProductDevRuntimeOutput::binding(replacement_runtime, CanonicalU64::new(0)),
                ProductDevRuntimeOutput::binding(replacement_runtime, CanonicalU64::new(0)),
            ],
        )
        .expect_err("second binding rejects the complete receipt");
        assert_eq!(error.code(), "DEV_HOST_OUTPUT_BASELINE");

        let locked = bus.lock().expect("test bus lock");
        assert_eq!(locked.events.len(), 1);
        let retained = locked.events.front().expect("retained event survives");
        assert_eq!(retained.id, retained_id);
        assert_eq!(retained.json.as_ptr(), retained_pointer);
        assert_eq!(retained.json.len(), retained_length);
        assert_eq!(locked.next_id, next_id);
        assert_eq!(locked.floor_cursor, floor_cursor);
        assert!(locked.active_binding.is_none());
        assert!(locked.pending_baseline.is_none());
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
        let active_runtime = binding();
        let replacement_runtime = crate::ProductDevRuntimeBinding {
            generation: CanonicalU64::new(2),
            ..active_runtime
        };
        let fragmented_payload =
            "x".repeat(MAX_OUTPUT_FRAGMENT_DATA_BYTES * (MAX_OUTPUT_QUEUE_ITEMS / 2 + 1));
        let bus = Mutex::new(OutputBus::default());
        push_outputs(
            &bus,
            vec![
                crate::model::ProductDevRuntimeOutput::binding(
                    active_runtime,
                    CanonicalU64::new(0),
                ),
                crate::model::ProductDevRuntimeOutput::complete_baseline(active_runtime),
            ],
        )
        .expect("initial runtime baseline publishes");
        let error = push_outputs(
            &bus,
            vec![
                crate::model::ProductDevRuntimeOutput::binding(
                    replacement_runtime,
                    CanonicalU64::new(0),
                ),
                crate::model::ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"payload": fragmented_payload}),
                ),
                crate::model::ProductDevRuntimeOutput::test_frame_value(
                    serde_json::json!({"payload": "y".repeat(
                        MAX_OUTPUT_FRAGMENT_DATA_BYTES * (MAX_OUTPUT_QUEUE_ITEMS / 2 + 1),
                    )}),
                ),
                crate::model::ProductDevRuntimeOutput::complete_baseline(replacement_runtime),
            ],
        )
        .expect_err("oversized complete producer baseline is rejected");
        assert_eq!(error.code(), "DEV_HOST_OUTPUT_BOUNDS");
        assert!(bus
            .lock()
            .expect("test bus lock")
            .pending_baseline
            .is_none());
        assert!(bus.lock().expect("test bus lock").active_binding.is_none());
        let incremental = push_outputs(
            &bus,
            vec![crate::model::ProductDevRuntimeOutput::test_frame_value(
                serde_json::json!({"incremental": true}),
            )],
        )
        .expect_err("incremental output cannot attach to a rejected baseline");
        assert_eq!(incremental.code(), "DEV_HOST_OUTPUT_BASELINE");
        assert_eq!(
            bus.lock().expect("test bus lock").events.len(),
            1,
            "replacement incremental was not mislabeled as the previous runtime",
        );
        push_outputs(
            &bus,
            vec![
                crate::model::ProductDevRuntimeOutput::binding(
                    replacement_runtime,
                    CanonicalU64::new(0),
                ),
                crate::model::ProductDevRuntimeOutput::complete_baseline(replacement_runtime),
            ],
        )
        .expect("a replayed full producer baseline publishes atomically");
        let locked = bus.lock().expect("test bus lock");
        assert!(locked.pending_baseline.is_none());
        assert_eq!(locked.active_binding, Some(replacement_runtime));
        assert_eq!(locked.events.len(), 2);
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
                ));
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
                ));
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
                ));
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
                ));
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
    content_type: String,
    body: Vec<u8>,
    output_through: Option<u64>,
    commit_disposition: Option<CommitDisposition>,
}

/// The host makes only two delivery claims for a typed runtime receipt.
/// `ResyncRequired` means the runtime result was committed, but the caller
/// must use its existing binding/readout identity and `/outputs/fresh` rather
/// than replaying the route request.
#[derive(Clone, Copy)]
enum CommitDisposition {
    Committed,
    ResyncRequired,
}

impl CommitDisposition {
    const fn as_header(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::ResyncRequired => "resync-required",
        }
    }
}

impl HttpResponse {
    fn bytes(status: u16, content_type: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body,
            output_through: None,
            commit_disposition: None,
        }
    }

    fn with_output_through(mut self, output_through: u64) -> Self {
        self.output_through = Some(output_through);
        self.commit_disposition = Some(CommitDisposition::Committed);
        self
    }

    fn with_committed(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::Committed);
        self
    }

    fn with_resync_required(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::ResyncRequired);
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
    if let Some(disposition) = response.commit_disposition {
        write!(
            stream,
            "X-Rusty-Commit-Disposition: {}\r\n",
            disposition.as_header()
        )?;
        if matches!(disposition, CommitDisposition::ResyncRequired) {
            stream.write_all(b"X-Rusty-Resync-Outputs: fresh\r\n")?;
        }
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
struct DiagnosticsReadRequest {
    after: Option<CanonicalU64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticsReadResponse {
    #[serde(flatten)]
    batch: crate::ProductDevLogBatch,
    telemetry: ProductDevTelemetrySnapshot,
}

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
