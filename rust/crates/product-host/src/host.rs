use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex, RwLock, Weak,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::{
    CanonicalU64, ProductHostBrowserConnectionState, ProductHostBrowserDiagnosticsReport,
    ProductHostBrowserDiagnosticsResult, ProductHostBrowserHostState, ProductHostBundle,
    ProductHostControlOperation, ProductHostError, ProductHostInputBatch, ProductHostInputResult,
    ProductHostLifecycleOperation, ProductHostLog, ProductHostLogDisposition, ProductHostLogEvent,
    ProductHostLogSeverity, ProductHostOperationKind, ProductHostOperationResult,
    ProductHostRuntime, ProductHostRuntimeError, ProductHostRuntimeOutput,
    ProductHostRuntimeReceipt, ProductHostTelemetrySnapshot, ProductHostTimelineCompletion,
    ProductHostUpdateAttribution, ProductHostUpdateAttributionSnapshot, MAX_CONNECTIONS,
    MAX_REQUEST_BODY_BYTES, MAX_REQUEST_HEADER_BYTES, MAX_SSE_SUBSCRIBERS,
    MAX_SUBSCRIBER_QUEUE_EVENTS,
};

use crate::frames::ProductHostFrameStream;
use crate::session::ProductHostOperationOwner;

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

/// Configuration for the fixed product host.
#[derive(Clone)]
pub struct ProductHostConfig {
    /// `0` asks the operating system for a free port.
    pub port: u16,
    pub bundle: ProductHostBundle,
    bind_host: Ipv4Addr,
    live_debug_enabled: bool,
    diagnostics: ProductHostLog,
    accept_decision_hook: Option<AcceptDecisionHook>,
    listener: Option<Arc<TcpListener>>,
    frames: Option<Arc<ProductHostFrameStream>>,
    capture: Option<crate::ProductHostFrameCapture>,
}

impl ProductHostConfig {
    pub fn new(port: u16, bundle: ProductHostBundle) -> Self {
        Self {
            port,
            bundle,
            bind_host: Ipv4Addr::LOCALHOST,
            live_debug_enabled: false,
            diagnostics: ProductHostLog::new(Default::default())
                .expect("fixed diagnostic defaults"),
            accept_decision_hook: None,
            listener: None,
            frames: None,
            capture: None,
        }
    }

    /// Serves on an already bound listener, such as one a supervising
    /// process bound and handed to this runtime process.
    pub fn with_listener(mut self, listener: TcpListener) -> Self {
        self.listener = Some(Arc::new(listener));
        self
    }

    /// Selects an explicit trusted development-network listener. Loopback is
    /// the default; `0.0.0.0` is intended for a foreground owner such as
    /// den-serve that publishes the resulting LAN origin.
    /// Serve the runtime's rendered frames at `/__rusty/product/runtime/frames`.
    pub fn with_frame_stream(mut self, frames: Arc<ProductHostFrameStream>) -> Self {
        self.frames = Some(frames);
        self
    }

    /// Serve tool captures at `/__rusty/product/runtime/frames/capture`, in
    /// either render output.
    pub fn with_frame_capture(mut self, capture: crate::ProductHostFrameCapture) -> Self {
        self.capture = Some(capture);
        self
    }

    pub fn with_bind_host(mut self, bind_host: Ipv4Addr) -> Self {
        self.bind_host = bind_host;
        self
    }

    /// Explicitly admits the trusted first-party product live-debug routes.
    /// They are absent by default so ordinary product hosts do not expose a
    /// command endpoint accidentally.
    pub fn with_live_debug(mut self, enabled: bool) -> Self {
        self.live_debug_enabled = enabled;
        self
    }

    pub fn with_diagnostics(mut self, diagnostics: ProductHostLog) -> Self {
        self.diagnostics = diagnostics;
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

/// Starts an Engine-owned local product host for one concrete runtime.
pub struct ProductHost;

impl ProductHost {
    pub fn start<R: ProductHostRuntime>(
        runtime: R,
        config: ProductHostConfig,
    ) -> Result<RunningProductHost, ProductHostError> {
        let listener = match &config.listener {
            Some(listener) => listener.try_clone(),
            None => TcpListener::bind(SocketAddr::from((config.bind_host, config.port))),
        }
        .map_err(|error| ProductHostError::io("PRODUCT_HOST_BIND", error))?;
        let address = listener
            .local_addr()
            .map_err(|error| ProductHostError::io("PRODUCT_HOST_ADDRESS", error))?;
        // Capture only the runtime's scheduler participation, not its
        // mutable lifecycle state. Input admission must never reacquire the
        // runtime owner behind a slow product update; Created/Paused realtime
        // products still retain their bounded mailbox for the next boundary.
        let realtime_scheduler_enabled = !matches!(
            runtime.realtime_schedule_state(),
            crate::ProductHostRuntimeScheduleState::Unsupported
        );
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler_wake = Arc::new(SchedulerWake::default());
        let output_wake = Arc::new(OutputWake::default());
        let bundle = Arc::new(RwLock::new(config.bundle));
        let outputs = Arc::new(Mutex::new(OutputBus::default()));
        let state = Arc::new(HostState {
            bundle: Arc::clone(&bundle),
            runtime: Arc::new(ProductHostOperationOwner::new(runtime)),
            input_mailbox: Arc::new(HostInputMailbox::default()),
            telemetry: Arc::new(Mutex::new(HostTelemetry::default())),
            realtime_scheduler_enabled,
            outputs: Arc::clone(&outputs),
            output_wake: Arc::clone(&output_wake),
            shutdown: Arc::clone(&shutdown),
            scheduler_wake: Arc::clone(&scheduler_wake),
            bind_host: config.bind_host,
            expected_port: address.port(),
            live_debug_enabled: config.live_debug_enabled,
            diagnostics: config.diagnostics.clone(),
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
            published_readout: Mutex::new(None),
            frames: config.frames,
            capture: config.capture,
        });
        let handler_threads = Arc::new(Mutex::new(Vec::new()));
        let listener_state = Arc::clone(&state);
        let listener_threads = Arc::clone(&handler_threads);
        let accept_decision_hook = config.accept_decision_hook;
        let listener_thread = thread::Builder::new()
            .name("rusty-product-host".to_owned())
            .spawn(move || {
                accept_loop(
                    listener,
                    listener_state,
                    listener_threads,
                    accept_decision_hook,
                )
            })
            .map_err(|error| ProductHostError::io("PRODUCT_HOST_THREAD", error))?;
        let scheduler_state = Arc::clone(&state);
        let scheduler_wake_thread = Arc::clone(&scheduler_wake);
        let scheduler_thread = match thread::Builder::new()
            .name("rusty-product-realtime-scheduler".to_owned())
            .spawn(move || scheduler_loop(scheduler_state, scheduler_wake_thread))
        {
            Ok(thread) => thread,
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
                return Err(ProductHostError::io("PRODUCT_HOST_THREAD", error));
            }
        };
        let content_owner = Arc::clone(&state.runtime);
        let asset_reload = ProductHostAssetReload {
            bundle,
            content: Arc::new(move || content_owner.reload_content()),
            outputs: Arc::clone(&state.outputs),
            output_wake: Arc::clone(&output_wake),
        };
        Ok(RunningProductHost {
            address,
            asset_reload,
            shutdown,
            scheduler_wake,
            output_wake,
            scheduler_thread: Some(scheduler_thread),
            listener_thread: Some(listener_thread),
            handler_threads,
            diagnostics: config.diagnostics,
        })
    }
}

/// A running product host. Shutdown is explicit and joins every accepted
/// connection handler so tests and generated launchers do not leak threads.
pub struct RunningProductHost {
    address: SocketAddr,
    asset_reload: ProductHostAssetReload,
    shutdown: Arc<AtomicBool>,
    scheduler_wake: Arc<SchedulerWake>,
    output_wake: Arc<OutputWake>,
    scheduler_thread: Option<JoinHandle<()>>,
    listener_thread: Option<JoinHandle<()>>,
    handler_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    diagnostics: ProductHostLog,
}

impl RunningProductHost {
    pub const fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    /// A handle that reloads UI and content into this running host.
    pub fn asset_reload(&self) -> ProductHostAssetReload {
        self.asset_reload.clone()
    }

    /// Whether the host has stopped serving.
    pub fn termination_requested(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn shutdown(mut self) -> Result<(), ProductHostError> {
        self.stop()
    }

    fn stop(&mut self) -> Result<(), ProductHostError> {
        let was_shutdown = self.shutdown.swap(true, Ordering::SeqCst);
        // Stop the host-owned realtime loop first. It may be in a product
        // callback, so joining it before connection handlers/runtime drop
        // preserves one clear teardown order and prevents post-shutdown work.
        self.scheduler_wake.notify();
        self.output_wake.notify();
        if let Some(thread) = self.scheduler_thread.take() {
            thread.join().map_err(|_| {
                ProductHostError::new("PRODUCT_HOST_THREAD_JOIN", "scheduler thread panicked")
            })?;
        }
        // Wake a nonblocking accept loop promptly. The connection is accepted
        // and observes the same shutdown flag before it parses a request.
        if !was_shutdown || self.listener_thread.is_some() {
            let _ = TcpStream::connect_timeout(&self.address, SOCKET_TIMEOUT);
        }
        if let Some(thread) = self.listener_thread.take() {
            thread.join().map_err(|_| {
                ProductHostError::new("PRODUCT_HOST_THREAD_JOIN", "listener thread panicked")
            })?;
        }
        let handlers = std::mem::take(&mut *self.handler_threads.lock().map_err(|_| {
            ProductHostError::new("PRODUCT_HOST_THREAD_JOIN", "handler thread ledger poisoned")
        })?);
        for handler in handlers {
            handler.join().map_err(|_| {
                ProductHostError::new("PRODUCT_HOST_THREAD_JOIN", "connection handler panicked")
            })?;
        }
        self.diagnostics.flush();
        Ok(())
    }
}

/// Swaps the served browser bundle and reloads the runtime's content without
/// restarting the product. UI is a mutable development route (`no-store`), so
/// the next page load gets the new files; content keeps its content-addressed
/// identities, so changed bytes get new URLs and old references keep theirs.
#[derive(Clone)]
pub struct ProductHostAssetReload {
    bundle: Arc<RwLock<ProductHostBundle>>,
    content: Arc<dyn Fn() -> Result<(), ProductHostRuntimeError> + Send + Sync>,
    outputs: Arc<Mutex<OutputBus>>,
    output_wake: Arc<OutputWake>,
}

impl ProductHostAssetReload {
    /// Content admission can fail, so it runs first; a failed reload leaves
    /// the old UI served with the old content. After a successful reload,
    /// each attached page is told to reload itself: it holds the UI module it
    /// loaded, and a headless page has no one to refresh it.
    pub fn reload(&self, bundle: ProductHostBundle) -> Result<(), ProductHostRuntimeError> {
        (self.content)()?;
        *self.bundle.write().map_err(|_| {
            ProductHostRuntimeError::new("PRODUCT_HOST_BUNDLE", "bundle lock poisoned")
        })? = bundle;
        if let Ok(mut outputs) = self.outputs.lock() {
            outputs.publish_ui_reloaded();
        }
        self.output_wake.notify();
        Ok(())
    }
}

impl Drop for RunningProductHost {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct HostState<R> {
    bundle: Arc<RwLock<ProductHostBundle>>,
    runtime: Arc<ProductHostOperationOwner<R>>,
    input_mailbox: Arc<HostInputMailbox>,
    telemetry: Arc<Mutex<HostTelemetry>>,
    realtime_scheduler_enabled: bool,
    outputs: Arc<Mutex<OutputBus>>,
    output_wake: Arc<OutputWake>,
    shutdown: Arc<AtomicBool>,
    scheduler_wake: Arc<SchedulerWake>,
    bind_host: Ipv4Addr,
    expected_port: u16,
    live_debug_enabled: bool,
    diagnostics: ProductHostLog,
    connections: AtomicUsize,
    subscribers: AtomicUsize,
    /// The last readout put on the output stream. Readouts are published
    /// only when they change what a browser shows, not every tick.
    published_readout: Mutex<Option<crate::ProductHostRuntimeReadout>>,
    frames: Option<Arc<ProductHostFrameStream>>,
    capture: Option<crate::ProductHostFrameCapture>,
}

/// Small process-local observation state. It intentionally has no runtime
/// reference: diagnostics can read it while the product owner is in a slow
/// callback and therefore never become another source of input backpressure.
#[derive(Default)]
struct HostTelemetry {
    in_flight_operation: Option<ProductHostOperationKind>,
    in_flight_started_ns: Option<u64>,
    last_product_admission_latency_ms: Option<u64>,
    last_input_admission_latency_ms: Option<u64>,
    progress_samples_ns: VecDeque<u64>,
    update_attribution_samples: VecDeque<(u64, ProductHostUpdateAttribution)>,
    slowest_update_attribution: Option<(u64, ProductHostUpdateAttribution)>,
}

impl HostTelemetry {
    const MAX_PROGRESS_SAMPLES: usize = 32;
    const PROGRESS_WINDOW_NS: u64 = 5_000_000_000;
    const MAX_UPDATE_ATTRIBUTION_SAMPLES: usize = 2_048;

    fn begin(&mut self, operation: ProductHostOperationKind, started_ns: u64) {
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
        sample: ProductHostUpdateAttribution,
    ) {
        if self
            .update_attribution_samples
            .back()
            .is_some_and(|(_, previous)| {
                previous
                    .runtime
                    .zip(sample.runtime)
                    .is_some_and(|(old, new)| {
                        old.instance_id != new.instance_id || old.generation != new.generation
                    })
            })
        {
            self.update_attribution_samples.clear();
            self.slowest_update_attribution = None;
            self.progress_samples_ns.clear();
        }
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
    ) -> Option<ProductHostUpdateAttributionSnapshot> {
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
        Some(ProductHostUpdateAttributionSnapshot {
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
    ) -> ProductHostTelemetrySnapshot {
        let progress_age_ms = self
            .progress_samples_ns
            .back()
            .map(|sample| now_ns.saturating_sub(*sample).saturating_div(1_000_000));
        let progress_rate_millihertz = match (
            self.progress_samples_ns.front(),
            self.progress_samples_ns.back(),
        ) {
            (Some(first), Some(last))
                if last > first && now_ns.saturating_sub(*last) <= Self::PROGRESS_WINDOW_NS =>
            {
                Some(CanonicalU64::new(
                    ((self.progress_samples_ns.len().saturating_sub(1) as u128)
                        .saturating_mul(1_000_000_000_000)
                        .saturating_div(u128::from(last - first)))
                    .min(u128::from(u64::MAX)) as u64,
                ))
            }
            _ => None,
        };
        ProductHostTelemetrySnapshot {
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
            runtime_progress_unavailable_reason: progress_rate_millihertz.is_none().then(|| {
                match progress_age_ms {
                    None => "No completed update observed in this incarnation",
                    Some(age) if age > Self::PROGRESS_WINDOW_NS / 1_000_000 => {
                        "No completed update in the last five seconds (paused, stopped, or busy)"
                    }
                    _ => "Waiting for a second completed update to measure progress",
                }
                .to_owned()
            }),
            connections: transport.connections,
            subscribers: transport.subscribers,
            output_queue_items: transport.output_queue_items,
            output_queue_capacity: MAX_SUBSCRIBER_QUEUE_EVENTS,
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
    batches: VecDeque<ProductHostInputBatch>,
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
    fn enqueue(&self, batch: ProductHostInputBatch, enqueued_ns: Option<u64>) -> bool {
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

    fn drain(&self) -> (Vec<ProductHostInputBatch>, bool) {
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

fn scheduler_loop<R: ProductHostRuntime>(state: Arc<HostState<R>>, wake: Arc<SchedulerWake>) {
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
                    ProductHostLogSeverity::Error,
                    ProductHostLogDisposition::Terminal,
                    error.code(),
                    error.diagnostic(),
                    [],
                );
                break;
            }
        };
        match schedule_state {
            crate::ProductHostRuntimeScheduleState::Unsupported => {
                wake.wait_timeout(SCHEDULER_IDLE_WAIT);
                continue;
            }
            crate::ProductHostRuntimeScheduleState::Shutdown => break,
            crate::ProductHostRuntimeScheduleState::Created
            | crate::ProductHostRuntimeScheduleState::Paused
            | crate::ProductHostRuntimeScheduleState::Faulted => {
                // Resume from a lifecycle transition at the next admitted
                // observation; this reset is only a phase marker, never a
                // substitute for the runtime's fixed-step interval.
                next_tick = Instant::now();
                wake.wait_timeout(SCHEDULER_IDLE_WAIT);
                continue;
            }
            crate::ProductHostRuntimeScheduleState::Running => {}
        }

        let Some(interval) = (match state.runtime.realtime_schedule_interval() {
            Ok(interval) => interval,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductHostLogSeverity::Error,
                    ProductHostLogDisposition::Terminal,
                    error.code(),
                    error.diagnostic(),
                    [],
                );
                break;
            }
        }) else {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Error,
                ProductHostLogDisposition::Terminal,
                "PRODUCT_HOST_SCHEDULER_CONFIGURATION",
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
        let input_errors = crate::advance_realtime_with_input_and_publish(
            &state.runtime,
            || state.input_mailbox.drain(),
            CanonicalU64::new(observed),
            |receipt| publish_scheduled_input_receipt(&state, receipt),
            |receipt| publish_scheduled_receipt(&state, receipt),
            || {
                begin_telemetry(&state, ProductHostOperationKind::AdvanceRealtime);
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
                        ProductHostLogSeverity::Warning,
                        disposition_for_runtime_error(&error),
                        error.code(),
                        error.diagnostic(),
                        [],
                    );
                }
            }
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductHostLogSeverity::Warning,
                    disposition_for_runtime_error(&error),
                    error.code(),
                    error.diagnostic(),
                    [],
                );
            }
        }
        let after = Instant::now();
        next_tick = next_tick
            .checked_add(fixed_interval)
            .filter(|deadline| *deadline > after)
            .unwrap_or_else(|| after + fixed_interval);
    }
}

fn disposition_for_runtime_error(error: &ProductHostRuntimeError) -> ProductHostLogDisposition {
    match crate::runtime_fault_disposition(error) {
        crate::ProductHostFaultDisposition::Accepted => ProductHostLogDisposition::Accepted,
        crate::ProductHostFaultDisposition::RejectedRecoverable => {
            ProductHostLogDisposition::RejectedRecoverable
        }
        crate::ProductHostFaultDisposition::Degraded => ProductHostLogDisposition::Degraded,
        crate::ProductHostFaultDisposition::ResyncRequired => {
            ProductHostLogDisposition::ResyncRequired
        }
        crate::ProductHostFaultDisposition::Terminal => ProductHostLogDisposition::Terminal,
    }
}

fn publish_scheduled_input_receipt<R: ProductHostRuntime>(
    state: &HostState<R>,
    receipt: crate::ProductHostRuntimeReceipt<ProductHostInputResult>,
) {
    let (result, mut outputs) = match receipt.into_wire_parts() {
        Ok(parts) => parts,
        Err(error) => {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Warning,
                ProductHostLogDisposition::ResyncRequired,
                "PRODUCT_HOST_SCHEDULE_INPUT_OUTPUT_RESYNC",
                "committed scheduled input receipt could not be encoded; reconnect for a fresh readout instead of replaying",
                [("cause", error.code().to_owned())],
            );
            return;
        }
    };
    // Input is admitted by the Rust-host scheduler rather than by the HTTP
    // request thread. Carry the complete typed result through the same
    // ordered SSE output family so accepted/consumed cursors and recoverable
    // stale drops remain observable without delaying POST acknowledgement.
    outputs.insert(0, ProductHostRuntimeOutput::runtime_input_result(result));
    if let Err(error) = push_host_outputs(state, outputs) {
        publish_host_diagnostic(
            &state.diagnostics,
            ProductHostLogSeverity::Warning,
            ProductHostLogDisposition::ResyncRequired,
            "PRODUCT_HOST_SCHEDULE_INPUT_OUTPUT_RESYNC",
            error.detail(),
            [("cause", error.code().to_owned())],
        );
    }
}

/// Returns a readout output when `readout` changes what a browser shows
/// relative to the last published one, and records it as published.
fn changed_readout<R: ProductHostRuntime>(
    state: &HostState<R>,
    readout: Option<&crate::ProductHostRuntimeReadout>,
) -> Option<ProductHostRuntimeOutput> {
    let readout = readout?;
    let mut published = state
        .published_readout
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if published
        .as_ref()
        .is_some_and(|previous| !readout.changes_browser_view(previous))
    {
        return None;
    }
    *published = Some(readout.clone());
    Some(ProductHostRuntimeOutput::runtime_readout(readout.clone()))
}

fn publish_scheduled_receipt<R: ProductHostRuntime>(
    state: &HostState<R>,
    receipt: crate::ProductHostRuntimeReceipt<ProductHostOperationResult>,
) {
    let progress_ns = state.diagnostics.now_monotonic_nanoseconds();
    if let Some(progress_ns) = progress_ns {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            telemetry.record_progress(progress_ns);
        }
    }
    let (result, mut outputs) = match receipt.into_wire_parts() {
        Ok(parts) => parts,
        Err(error) => {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Warning,
                ProductHostLogDisposition::ResyncRequired,
                "PRODUCT_HOST_SCHEDULE_OUTPUT_RESYNC",
                "committed scheduled receipt could not be encoded; reconnect for a fresh readout instead of replaying",
                [("cause", error.code().to_owned())],
            );
            return;
        }
    };
    if let Some(readout) = changed_readout(state, result.readout()) {
        outputs.push(readout);
    }
    if outputs.is_empty() {
        return;
    }
    if let Err(error) = push_host_outputs(state, outputs) {
        publish_host_diagnostic(
            &state.diagnostics,
            ProductHostLogSeverity::Warning,
            ProductHostLogDisposition::ResyncRequired,
            "PRODUCT_HOST_SCHEDULE_OUTPUT_RESYNC",
            error.detail(),
            [("cause", error.code().to_owned())],
        );
    }
}

fn accept_loop<R: ProductHostRuntime>(
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
                            "PRODUCT_HOST_CONNECTION_BOUNDS",
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
                        ProductHostLogSeverity::Warning,
                        ProductHostLogDisposition::RejectedRecoverable,
                        "PRODUCT_HOST_LISTENER_ACCEPT_RETRY",
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
                        ProductHostLogSeverity::Error,
                        ProductHostLogDisposition::Terminal,
                        "PRODUCT_HOST_LISTENER_ACCEPT_TERMINAL",
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
    diagnostics: &ProductHostLog,
    severity: ProductHostLogSeverity,
    disposition: ProductHostLogDisposition,
    code: &str,
    message: &str,
    fields: [(&str, String); N],
) {
    let Ok(mut event) =
        ProductHostLogEvent::new(severity, disposition, "product-host", code, message)
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

fn handle_connection<R: ProductHostRuntime>(mut stream: TcpStream, state: Arc<HostState<R>>) {
    if state.shutdown.load(Ordering::Acquire) {
        return;
    }
    let peer = stream
        .peer_addr()
        .map_or_else(|_| "unknown".to_owned(), |peer| peer.to_string());
    let request = match read_request(&mut stream) {
        Ok(request) => request,
        Err(response) => {
            // No request reached dispatch. A speculative browser connection may
            // close normally, so retain attribution without making it a warning.
            // Do not use RejectedRecoverable: its once-per-code admission
            // warning policy would discard the later socket attribution.
            // This body is our fixed parser diagnostic, never request contents.
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Info,
                ProductHostLogDisposition::Degraded,
                "PRODUCT_HOST_REQUEST_READ_REJECTED",
                "connection ended or failed before a complete request reached dispatch",
                [
                    ("peer", peer),
                    ("http-status", response.status.to_string()),
                    (
                        "diagnostic",
                        String::from_utf8_lossy(&response.body).into_owned(),
                    ),
                ],
            );
            let _ = write_response(&mut stream, response);
            return;
        }
    };
    if !has_admitted_origin(&request, state.bind_host, state.expected_port) {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(
                400,
                "PRODUCT_HOST_ORIGIN",
                "Host or Origin is not this product host origin",
            ),
        );
        return;
    }
    if request.method == "GET" && request.path == "/__rusty/product/runtime/outputs/fresh" {
        handle_sse(stream, state, request);
        return;
    }
    if request.method == "GET"
        && request
            .path
            .split('?')
            .next()
            .is_some_and(|route| route == crate::frames::PRODUCT_HOST_FRAMES_PATH)
    {
        handle_frames(stream, &state, &request);
        return;
    }
    if request.method == "GET"
        && request
            .path
            .split('?')
            .next()
            .is_some_and(|route| route == crate::frames::PRODUCT_HOST_FRAME_CAPTURE_PATH)
    {
        let response = capture_response(&state, &request);
        let _ = write_response(&mut stream, response);
        return;
    }
    // Preserve the browser attachment correlation before dispatch consumes the
    // request. A response write may fail only after a route has admitted work.
    let request_path = request.path.clone();
    let attachment_id = request
        .headers
        .get("x-rusty-browser-attachment")
        .filter(|value| crate::model::browser_attachment_id_is_admitted(value))
        .cloned()
        .unwrap_or_else(|| "none".to_owned());
    let response = dispatch_request(&state, request);
    let delivery_certainty = response.delivery_certainty;
    let response_status = response.status;
    if let Err(error) = write_response(&mut stream, response) {
        if let Some(certainty) = delivery_certainty {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Warning,
                ProductHostLogDisposition::ResyncRequired,
                "PRODUCT_HOST_RESPONSE_WRITE_RESYNC",
                "response delivery failed after a confirmed host admission; preserve its delivery certainty and do not replay the request",
                [
                    ("error-kind", format!("{:?}", error.kind())),
                    ("peer", peer),
                    ("attachment-id", attachment_id),
                    ("request-path", request_path),
                    ("response-certainty", certainty.as_field().to_owned()),
                ],
            );
        } else {
            publish_host_diagnostic(
                &state.diagnostics,
                ProductHostLogSeverity::Info,
                ProductHostLogDisposition::Degraded,
                "PRODUCT_HOST_RESPONSE_WRITE_UNAVAILABLE",
                "response socket failed without a confirmed admission receipt; do not infer mutation outcome",
                [
                    ("error-kind", format!("{:?}", error.kind())),
                    ("peer", peer),
                    ("attachment-id", attachment_id),
                    ("request-path", request_path),
                    ("http-status", response_status.to_string()),
                ],
            );
        }
    }
}

fn dispatch_request<R: ProductHostRuntime>(
    state: &HostState<R>,
    request: HttpRequest,
) -> HttpResponse {
    if request.method == "GET" {
        if request.path == "/__rusty/product/runtime/debug/catalog" {
            if !state.live_debug_enabled {
                return HttpResponse::error(
                    404,
                    "PRODUCT_HOST_ROUTE_NOT_FOUND",
                    "route is not admitted",
                );
            }
            if !request.body.is_empty() {
                return HttpResponse::error(
                    400,
                    "PRODUCT_HOST_GET_BODY",
                    "GET requests cannot carry a body",
                );
            }
            return invoke_debug_catalog(state);
        }
        if request.body.is_empty() {
            if let Ok(bundle) = state.bundle.read() {
                if let Some(response) = bundle_response(&bundle, &request.path) {
                    return response;
                }
            }
            return HttpResponse::error(
                404,
                "PRODUCT_HOST_ROUTE_NOT_FOUND",
                "route is not admitted",
            );
        }
        return HttpResponse::error(
            400,
            "PRODUCT_HOST_GET_BODY",
            "GET requests cannot carry a body",
        );
    }
    if request.method != "POST" {
        return HttpResponse::error(
            405,
            "PRODUCT_HOST_METHOD",
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
            "PRODUCT_HOST_CONTENT_TYPE",
            "POST requests require application/json",
        );
    }
    match request.path.as_str() {
        "/__rusty/product/runtime/lifecycle/start" => {
            invoke_lifecycle(state, &request.body, ProductHostLifecycleOperation::Start)
        }
        "/__rusty/product/runtime/lifecycle/pause" => {
            invoke_lifecycle(state, &request.body, ProductHostLifecycleOperation::Pause)
        }
        "/__rusty/product/runtime/lifecycle/resume" => {
            invoke_lifecycle(state, &request.body, ProductHostLifecycleOperation::Resume)
        }
        "/__rusty/product/runtime/lifecycle/restart" => {
            invoke_lifecycle(state, &request.body, ProductHostLifecycleOperation::Restart)
        }
        "/__rusty/product/runtime/lifecycle/shutdown" => invoke_lifecycle(
            state,
            &request.body,
            ProductHostLifecycleOperation::Shutdown,
        ),
        "/__rusty/product/runtime/lifecycle/report-fault" => invoke_lifecycle(
            state,
            &request.body,
            ProductHostLifecycleOperation::ReportFault,
        ),
        "/__rusty/product/runtime/control/replace" => {
            invoke_control(state, &request.body, ProductHostControlOperation::Replace)
        }
        "/__rusty/product/runtime/control/release" => {
            invoke_control(state, &request.body, ProductHostControlOperation::Release)
        }
        "/__rusty/product/runtime/control/claim" => invoke_claim(state, &request.body),
        "/__rusty/product/runtime/input" => invoke_input(state, &request.body),
        "/__rusty/product/runtime/advance-realtime" => invoke_realtime(state, &request.body),
        "/__rusty/product/runtime/admit-demand-step" => invoke_demand(state, &request.body),
        "/__rusty/product/runtime/admit-external-step" => invoke_external(state, &request.body),
        "/__rusty/product/runtime/timeline-completion" => invoke_timeline(state, &request.body),
        "/__rusty/product/runtime/diagnostics/read" => {
            invoke_diagnostics_read(state, &request.body)
        }
        "/__rusty/product/runtime/browser-diagnostics" => {
            invoke_browser_diagnostics(state, &request.body)
        }
        _ => HttpResponse::error(404, "PRODUCT_HOST_ROUTE_NOT_FOUND", "route is not admitted"),
    }
}

/// Serve an exact bundle path.
fn bundle_response(bundle: &ProductHostBundle, request_path: &str) -> Option<HttpResponse> {
    let entry = bundle.get(request_path)?;
    Some(HttpResponse::bytes(
        200,
        entry.content_type(),
        entry.shared_bytes(),
    ))
}

fn invoke_debug_catalog<R: ProductHostRuntime>(state: &HostState<R>) -> HttpResponse {
    match state
        .runtime
        .session()
        .with_locked_timed(
            || begin_telemetry(state, ProductHostOperationKind::ExecuteDebug),
            |runtime| {
                let result = runtime.describe_debug();
                let receipt = match result {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        return Ok(HttpResponse::error(500, error.code(), error.diagnostic()));
                    }
                };
                let (catalog, outputs) = match receipt.into_wire_parts() {
                    Ok(parts) => parts,
                    Err(error) => {
                        return Ok(HttpResponse::error(503, error.code(), error.detail()));
                    }
                };
                let output_through = match push_host_outputs(state, outputs) {
                    Ok(output_through) => output_through,
                    Err(error) => {
                        return Ok(HttpResponse::error(503, error.code(), error.detail()));
                    }
                };
                Ok(json_response(200, &catalog).with_output_through(output_through))
            },
            || finish_telemetry(state, ProductHostOperationKind::ExecuteDebug),
        )
        .map_err(|_| crate::session::runtime_poisoned())
        .and_then(|response| response)
    {
        Ok(response) => response,
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
}

fn invoke_debug_execute<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let command = match std::str::from_utf8(body) {
        Ok(command) => command,
        Err(_) => return debug_text_error(400, "debug command body must be valid UTF-8"),
    };
    match state
        .runtime
        .session()
        .with_locked_timed(
            || begin_telemetry(state, ProductHostOperationKind::ExecuteDebug),
            |runtime| {
                if state.realtime_scheduler_enabled {
                    // Input accepted before this command reaches the runtime
                    // first; `engine.time.advance` steps held time with it.
                    let errors = crate::scheduler::deliver_queued_input(
                        runtime,
                        state.input_mailbox.drain(),
                        &mut |receipt| publish_scheduled_input_receipt(state, receipt),
                        &mut |receipt| publish_scheduled_receipt(state, receipt),
                    );
                    for error in errors {
                        publish_host_diagnostic(
                            &state.diagnostics,
                            ProductHostLogSeverity::Warning,
                            disposition_for_runtime_error(&error),
                            error.code(),
                            error.diagnostic(),
                            [],
                        );
                    }
                }
                let result = runtime.execute_debug(command);
                let receipt = match result {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        return Ok(debug_text_error(
                            500,
                            &format!("{}: {}", error.code(), error.diagnostic()),
                        ));
                    }
                };
                let (result, mut outputs) = match receipt.into_wire_parts() {
                    Ok(parts) => parts,
                    Err(error) => {
                        return Ok(debug_text_error(
                            503,
                            &format!("{}: {}", error.code(), error.detail()),
                        ));
                    }
                };
                if let Some(readout) = changed_readout(state, result.readout()) {
                    outputs.push(readout);
                }
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
            || finish_telemetry(state, ProductHostOperationKind::ExecuteDebug),
        )
        .map_err(|_| crate::session::runtime_poisoned())
        .and_then(|response| response)
    {
        Ok(response) => response,
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
}

fn invoke_lifecycle<R: ProductHostRuntime>(
    state: &HostState<R>,
    body: &[u8],
    operation: ProductHostLifecycleOperation,
) -> HttpResponse {
    let request: ProductHostLifecycleRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let response = call_runtime(
        state,
        operation.operation_kind(),
        |runtime| {
            let receipt = runtime.lifecycle_with_binding(operation, request.runtime)?;
            if receipt.result().is_accepted() {
                state.input_mailbox.clear();
            }
            Ok(receipt)
        },
        |error| ProductHostOperationResult::rejected_runtime(operation.operation_kind(), error),
    );
    state.scheduler_wake.notify();
    response
}

fn invoke_control<R: ProductHostRuntime>(
    state: &HostState<R>,
    body: &[u8],
    operation: ProductHostControlOperation,
) -> HttpResponse {
    let request: ProductHostControlRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let response = call_runtime(
        state,
        operation.operation_kind(),
        |runtime| {
            let receipt = runtime.control(operation, request.runtime)?;
            if receipt.result().is_accepted() {
                state.input_mailbox.clear();
            }
            Ok(receipt)
        },
        |error| ProductHostOperationResult::rejected_runtime(operation.operation_kind(), error),
    );
    state.scheduler_wake.notify();
    response
}

fn invoke_claim<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: ProductHostControlClaimRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let lease = std::time::Duration::from_millis(request.lease_ms.get());
    let response = call_runtime(
        state,
        ProductHostOperationKind::ClaimControl,
        |runtime| {
            let receipt = runtime.claim_control(request.runtime, request.label.clone(), lease)?;
            if receipt.result().is_accepted() {
                state.input_mailbox.clear();
            }
            Ok(receipt)
        },
        |error| {
            ProductHostOperationResult::rejected_runtime(
                ProductHostOperationKind::ClaimControl,
                error,
            )
        },
    );
    state.scheduler_wake.notify();
    response
}

fn invoke_input<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: ProductHostInputRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let batch_json = match serde_json::to_vec(&request.batch) {
        Ok(value) => value,
        Err(_) => {
            return HttpResponse::error(
                400,
                "PRODUCT_HOST_INPUT_DECODE",
                "input batch could not be encoded",
            );
        }
    };
    let batch = match ProductHostInputBatch::decode_json(&batch_json) {
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
                match ProductHostInputResult::queued(count) {
                    // Admission committed to the mailbox; runtime consumption
                    // and its output cursor arrive separately through SSE.
                    Ok(result) => json_response(200, &result).with_queued_input(),
                    Err(error) => HttpResponse::error(500, error.code(), error.detail()),
                }
            }
            false => {
                state.scheduler_wake.notify();
                match ProductHostInputResult::mailbox_full(count) {
                    Ok(result) => json_response(200, &result).with_resync_required(),
                    Err(error) => HttpResponse::error(500, error.code(), error.detail()),
                }
            }
        };
    }
    call_runtime(
        state,
        ProductHostOperationKind::Input,
        |runtime| runtime.input(batch),
        crate::ProductHostInputResult::rejected_runtime,
    )
}

fn recover_rejected_input_batch<R: ProductHostRuntime>(
    state: &HostState<R>,
    count: usize,
) -> HttpResponse {
    state.input_mailbox.clear();
    publish_host_diagnostic(
        &state.diagnostics,
        ProductHostLogSeverity::Warning,
        ProductHostLogDisposition::ResyncRequired,
        "PRODUCT_HOST_INPUT_DECODE",
        "input batch was not a strict runtime-input wire batch; replacing the runtime input binding before continuing",
        [("submitted-count", count.to_string())],
    );
    let response = call_runtime(
        state,
        ProductHostOperationKind::Input,
        |runtime| {
            let recovery = runtime.recover_input_overflow()?;
            let (_operation, outputs) = recovery.into_parts();
            let result = ProductHostInputResult::wire_decode_resynchronized(count)
                .map_err(host_error_to_runtime)?;
            ProductHostRuntimeReceipt::new(result, outputs).map_err(host_error_to_runtime)
        },
        ProductHostInputResult::rejected_runtime,
    );
    state.scheduler_wake.notify();
    response
}

fn host_error_to_runtime(error: ProductHostError) -> ProductHostRuntimeError {
    ProductHostRuntimeError::new(error.code(), error.detail())
}

fn invoke_realtime<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: ProductHostRealtimeRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        ProductHostOperationKind::AdvanceRealtime,
        |runtime| runtime.advance_realtime(request.observed_time_ns),
        |error| {
            ProductHostOperationResult::rejected_runtime(
                ProductHostOperationKind::AdvanceRealtime,
                error,
            )
        },
    )
}

fn invoke_demand<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    if decode_empty(body).is_err() {
        return HttpResponse::error(
            400,
            "PRODUCT_HOST_REQUEST_BODY",
            "demand route requires exactly {} JSON",
        );
    }
    call_runtime(
        state,
        ProductHostOperationKind::AdmitDemandStep,
        |runtime| runtime.admit_demand_step(),
        |error| {
            ProductHostOperationResult::rejected_runtime(
                ProductHostOperationKind::AdmitDemandStep,
                error,
            )
        },
    )
}

fn invoke_external<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request: ProductHostExternalRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    call_runtime(
        state,
        ProductHostOperationKind::AdmitExternalStep,
        |runtime| runtime.admit_external_step(request.step),
        |error| {
            ProductHostOperationResult::rejected_runtime(
                ProductHostOperationKind::AdmitExternalStep,
                error,
            )
        },
    )
}

fn invoke_timeline<R: ProductHostRuntime>(state: &HostState<R>, body: &[u8]) -> HttpResponse {
    let request = match ProductHostTimelineCompletion::decode_json(body) {
        Ok(value) => value,
        Err(error) => return HttpResponse::error(400, error.code(), error.detail()),
    };
    let ticket = request.envelope().ticket().value();
    call_runtime(
        state,
        ProductHostOperationKind::CompleteTimeline,
        |runtime| runtime.complete_timeline(request),
        |error| {
            crate::ProductHostTimelineCompletionResult::rejected_runtime(
                CanonicalU64::new(ticket),
                error,
            )
        },
    )
}

fn invoke_diagnostics_read<R: ProductHostRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    if !state.live_debug_enabled {
        return HttpResponse::error(404, "PRODUCT_HOST_ROUTE_NOT_FOUND", "route is not admitted");
    }
    let request: ProductHostDiagnosticsReadRequest = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let batch = state
        .diagnostics
        .read_after(request.after.map(CanonicalU64::get));
    let telemetry = telemetry_snapshot(state, batch.read_monotonic_nanoseconds);
    json_response(
        200,
        &ProductHostDiagnosticsReadResponse { batch, telemetry },
    )
}

fn telemetry_snapshot<R: ProductHostRuntime>(
    state: &HostState<R>,
    now_ns: u64,
) -> ProductHostTelemetrySnapshot {
    let input = state.input_mailbox.telemetry();
    let transport = state
        .outputs
        .lock()
        .map(|outputs| TransportTelemetry {
            connections: state.connections.load(Ordering::Acquire),
            subscribers: state.subscribers.load(Ordering::Acquire),
            output_queue_items: outputs.largest_backlog(),
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

fn invoke_browser_diagnostics<R: ProductHostRuntime>(
    state: &HostState<R>,
    body: &[u8],
) -> HttpResponse {
    let report: ProductHostBrowserDiagnosticsReport = match decode_json(body) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = report.validate() {
        return HttpResponse::error(400, error.code(), error.detail());
    }
    let mut reported = 0_u8;
    let status_disposition = if matches!(
        report.host_state,
        ProductHostBrowserHostState::Degraded | ProductHostBrowserHostState::Failed
    ) {
        ProductHostLogDisposition::Degraded
    } else {
        ProductHostLogDisposition::Accepted
    };
    let status_severity = if matches!(
        report.host_state,
        ProductHostBrowserHostState::Degraded | ProductHostBrowserHostState::Failed
    ) {
        ProductHostLogSeverity::Warning
    } else {
        ProductHostLogSeverity::Info
    };
    let mut status = match ProductHostLogEvent::new(
        status_severity,
        status_disposition,
        "browser-host",
        "BROWSER_HOST_STATUS",
        "Product Browser Host transition snapshot",
    ) {
        Ok(event) => event,
        Err(error) => return HttpResponse::error(500, error.code(), error.detail()),
    };
    let established_baseline = matches!(report.host_state, ProductHostBrowserHostState::Ready)
        && matches!(
            report.transport_state,
            ProductHostBrowserConnectionState::Open
        )
        && matches!(report.output_state, ProductHostBrowserConnectionState::Open)
        && report.first_terminal.is_none()
        && report
            .attachment
            .as_ref()
            .and_then(|attachment| attachment.baseline.as_ref())
            .is_some();
    let attachment_id = report
        .attachment
        .as_ref()
        .map_or_else(|| "none".to_owned(), |attachment| attachment.id.clone());
    let replaces_attachment_id = report
        .attachment
        .as_ref()
        .and_then(|attachment| attachment.replaces.clone());
    let baseline_facts = if established_baseline {
        let baseline = report
            .attachment
            .as_ref()
            .and_then(|attachment| attachment.baseline.as_ref())
            .expect("established baseline requires a baseline");
        format!(
            "runtime={}/{}/{};next-input={}",
            baseline.runtime.instance_id.get(),
            baseline.runtime.generation.get(),
            baseline.runtime.control_revision.get(),
            baseline.next_input_sequence.get(),
        )
    } else {
        "none".to_owned()
    };
    for (key, value) in [
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
        ("attachment-id", attachment_id),
        (
            "replaces-attachment-id",
            replaces_attachment_id.unwrap_or_else(|| "none".to_owned()),
        ),
        ("baseline-established", established_baseline.to_string()),
        ("baseline-facts", baseline_facts),
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
        let event = match ProductHostLogEvent::new(
            ProductHostLogSeverity::Error,
            ProductHostLogDisposition::Terminal,
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
        let event = match ProductHostLogEvent::new(
            ProductHostLogSeverity::Warning,
            ProductHostLogDisposition::RejectedRecoverable,
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
        let event = match ProductHostLogEvent::new(
            ProductHostLogSeverity::Warning,
            ProductHostLogDisposition::Degraded,
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
        &ProductHostBrowserDiagnosticsResult {
            accepted: true,
            reported,
        },
    )
    // Browser diagnostics are an observation acknowledgement. They do not
    // settle a runtime receipt or a queued input admission.
    .with_observation()
}

fn browser_host_state(state: ProductHostBrowserHostState) -> &'static str {
    match state {
        ProductHostBrowserHostState::Loading => "loading",
        ProductHostBrowserHostState::Ready => "ready",
        ProductHostBrowserHostState::Degraded => "degraded",
        ProductHostBrowserHostState::Failed => "failed",
        ProductHostBrowserHostState::Disposed => "disposed",
    }
}

fn browser_connection_state(state: ProductHostBrowserConnectionState) -> &'static str {
    match state {
        ProductHostBrowserConnectionState::Open => "open",
        ProductHostBrowserConnectionState::Closed => "closed",
    }
}

fn browser_page_diagnostic_kind(kind: crate::ProductHostBrowserPageDiagnosticKind) -> &'static str {
    match kind {
        crate::ProductHostBrowserPageDiagnosticKind::Error => "error",
        crate::ProductHostBrowserPageDiagnosticKind::UnhandledRejection => "unhandled-rejection",
    }
}

fn begin_telemetry<R: ProductHostRuntime>(
    state: &HostState<R>,
    operation: ProductHostOperationKind,
) {
    if let Some(started_ns) = state.diagnostics.now_monotonic_nanoseconds() {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            telemetry.begin(operation, started_ns);
        }
    }
}

fn finish_telemetry<R: ProductHostRuntime>(
    state: &HostState<R>,
    operation: ProductHostOperationKind,
) {
    if let Some(finished_ns) = state.diagnostics.now_monotonic_nanoseconds() {
        if let Ok(mut telemetry) = state.telemetry.lock() {
            if let Some(latency_ms) = telemetry.finish(finished_ns) {
                if matches!(operation, ProductHostOperationKind::Input) {
                    telemetry.record_input_admission(latency_ms);
                }
            }
        }
    }
}

fn record_update_attribution<R: ProductHostRuntime>(
    state: &HostState<R>,
    attribution: Option<ProductHostUpdateAttribution>,
) {
    let Some(attribution) = attribution else {
        return;
    };
    let Some(completed_ns) = state.diagnostics.now_monotonic_nanoseconds() else {
        return;
    };
    if let Ok(mut telemetry) = state.telemetry.lock() {
        telemetry.record_update_attribution(completed_ns, attribution);
        if !state.realtime_scheduler_enabled {
            telemetry.record_progress(completed_ns);
        }
    }
}

fn call_runtime<R, T, F, E>(
    state: &HostState<R>,
    operation: ProductHostOperationKind,
    call: F,
    error_result: E,
) -> HttpResponse
where
    R: ProductHostRuntime,
    T: Serialize,
    F: FnOnce(&mut R) -> Result<crate::ProductHostRuntimeReceipt<T>, ProductHostRuntimeError>,
    E: FnOnce(ProductHostRuntimeError) -> Result<T, ProductHostError>,
{
    let response = state
        .runtime
        .session()
        .with_locked_timed(
        || begin_telemetry(state, operation),
        |runtime| {
        let call_result = call(runtime);
        let receipt = match call_result {
            Ok(receipt) => receipt,
            Err(error) => {
                let message = if error.diagnostic().is_empty() {
                    "runtime operation failed"
                } else {
                    error.diagnostic()
                };
                let _ = state.diagnostics.publish(
                    crate::ProductHostLogEvent::new(
                        crate::ProductHostLogSeverity::Error,
                        disposition_for_runtime_error(&error),
                        "runtime",
                        error.code(),
                        message,
                    )
                    .expect("runtime diagnostics are bounded"),
                );
                let disposition = error.disposition();
                return Ok((match error_result(error) {
                    Ok(result) => json_response(200, &result).with_runtime_error(disposition),
                    Err(host_error) => {
                        HttpResponse::error(500, host_error.code(), host_error.detail())
                    }
                }, runtime.take_update_attribution()));
            }
        };
        // Encoding is a host-owned preflight. A bad response must not first
        // publish retained output for a runtime mutation the client cannot name.
        let encoded_result = match encode_runtime_result(receipt.result()) {
            Ok(bytes) => bytes,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductHostLogSeverity::Warning,
                    ProductHostLogDisposition::ResyncRequired,
                    "PRODUCT_HOST_RESPONSE_ENCODE_RESYNC",
                    "runtime result could not be encoded after mutation; reconnect for a fresh readout instead of replaying",
                    [("cause", error.code().to_owned())],
                );
                return Ok((HttpResponse::error(500, error.code(), error.detail()), runtime.take_update_attribution()));
            }
        };
        let (_result, outputs) = match receipt.into_wire_parts() {
            Ok(parts) => parts,
            Err(error) => {
                publish_host_diagnostic(
                    &state.diagnostics,
                    ProductHostLogSeverity::Warning,
                    ProductHostLogDisposition::ResyncRequired,
                    "PRODUCT_HOST_OUTPUT_COMMIT_RESYNC",
                    "committed runtime receipt could not be encoded for output publication; reconnect for a fresh readout instead of replaying",
                    [("cause", error.code().to_owned())],
                );
                return Ok((HttpResponse::bytes(200, "application/json", encoded_result)
                    .with_settled_resync_required(), runtime.take_update_attribution()));
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
                    ProductHostLogSeverity::Warning,
                    ProductHostLogDisposition::ResyncRequired,
                    "PRODUCT_HOST_OUTPUT_COMMIT_RESYNC",
                    "retained output publication failed after an authoritative runtime receipt; reconnect for a fresh readout instead of replaying",
                    [("cause", error.code().to_owned())],
                );
                return Ok((HttpResponse::bytes(200, "application/json", encoded_result)
                    .with_settled_resync_required(), runtime.take_update_attribution()));
            }
        };
        Ok((HttpResponse::bytes(200, "application/json", encoded_result)
            .with_output_through(output_through), runtime.take_update_attribution()))
        },
        || finish_telemetry(state, operation),
    )
    .map_err(|_| crate::session::runtime_poisoned())
    .and_then(|response| response);
    match response {
        Ok((response, attribution)) => {
            record_update_attribution(state, attribution);
            response
        }
        Err(error) => HttpResponse::error(500, error.code(), error.diagnostic()),
    }
}

fn encode_runtime_result<T: Serialize>(value: &T) -> Result<Vec<u8>, ProductHostError> {
    match serde_json::to_vec(value) {
        Ok(bytes) if bytes.len() <= MAX_REQUEST_BODY_BYTES => Ok(bytes),
        Ok(_) => Err(ProductHostError::new(
            "PRODUCT_HOST_RESPONSE_BOUNDS",
            "runtime result exceeds response bound",
        )),
        Err(_) => Err(ProductHostError::new(
            "PRODUCT_HOST_RESPONSE_ENCODE",
            "runtime result could not be encoded",
        )),
    }
}

fn handle_frames<R: ProductHostRuntime>(
    mut stream: TcpStream,
    state: &HostState<R>,
    request: &HttpRequest,
) {
    let response = match (&state.frames, crate::frames::frame_request(&request.path)) {
        (None, _) => HttpResponse::error(
            404,
            "PRODUCT_HOST_FRAMES",
            "this runtime does not render frames",
        ),
        (Some(_), Err(detail)) => HttpResponse::error(400, "PRODUCT_HOST_FRAMES_REQUEST", detail),
        (Some(frames), Ok(frame_request)) => {
            if let Some(ratio) = frame_request.pixel_ratio {
                frames.set_viewer_pixel_ratio(ratio);
            }
            let frame = frames.next_after(
                frame_request.after,
                frame_request.size,
                crate::frames::FRAME_REQUEST_WAIT,
            );
            match frame {
                Some(frame) => HttpResponse::bytes(200, "application/x-rusty-frame", frame),
                None => HttpResponse::bytes(204, "application/x-rusty-frame", Vec::new()),
            }
        }
    };
    let _ = stream.set_nodelay(true);
    let _ = write_response(&mut stream, response);
}

/// A tool's capture: one frame drawn at its own size, never a viewer. The
/// drawn cameras ride in `X-Rusty-Frame-Cameras`.
fn capture_response<R: ProductHostRuntime>(
    state: &HostState<R>,
    request: &HttpRequest,
) -> HttpResponse {
    let Some(capture) = &state.capture else {
        return HttpResponse::error(
            404,
            "PRODUCT_HOST_FRAMES",
            "this runtime does not render frames",
        );
    };
    let request = match crate::frames::capture_request(&request.path) {
        Ok(request) => request,
        Err(detail) => return HttpResponse::error(400, "PRODUCT_HOST_CAPTURE_REQUEST", detail),
    };
    match capture(request) {
        Ok(captured) => {
            let mut response = HttpResponse::bytes(
                200,
                "application/x-rusty-frame",
                crate::frames::encode_capture(&captured),
            );
            response.cameras = Some(captured.cameras.to_string());
            response
        }
        Err(detail) => HttpResponse::error(500, "PRODUCT_HOST_CAPTURE", &detail),
    }
}

fn handle_sse<R: ProductHostRuntime>(
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
                "PRODUCT_HOST_SSE_REQUEST",
                "outputs requires empty EventSource GET",
            ),
        );
        return;
    }
    // Publish each small real-time event without waiting for an ACK of its
    // predecessor. Flush alone does not disable TCP Nagle.
    if stream.set_nodelay(true).is_err() {
        return;
    }
    if !try_acquire(&state.subscribers, MAX_SSE_SUBSCRIBERS) {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(
                503,
                "PRODUCT_HOST_SSE_BOUNDS",
                "SSE subscriber limit reached",
            ),
        );
        return;
    }
    let _subscriber = CounterGuard::new(&state.subscribers);
    // Every connection starts from a fresh baseline; there is no resume. The
    // subscriber's live queue starts at the moment its baseline is captured.
    let connection = state
        .runtime
        .session()
        .with_locked_timed(
            || begin_telemetry(&state, ProductHostOperationKind::Connect),
            |runtime| {
                let result = runtime.connect();
                let receipt = result?;
                let (result, mut outputs) = match receipt.into_wire_parts() {
                    Ok(parts) => parts,
                    Err(error) => {
                        return Ok(Err(HttpResponse::error(503, error.code(), error.detail())));
                    }
                };
                // Readouts are published on change, so a fresh subscriber
                // starts from the current one.
                if let Some(readout) = result.readout() {
                    outputs.push(ProductHostRuntimeOutput::runtime_readout(readout.clone()));
                }
                let (baseline, binding) = match encode_output_batches(None, outputs) {
                    Ok(encoded) => encoded,
                    Err(error) => {
                        return Ok(Err(HttpResponse::error(503, error.code(), error.detail())));
                    }
                };
                let Some(binding) = binding else {
                    return Ok(Err(HttpResponse::error(
                        503,
                        "PRODUCT_HOST_OUTPUT_BASELINE",
                        "runtime connection did not publish a complete binding baseline",
                    )));
                };
                let Ok(mut outputs) = state.outputs.lock() else {
                    return Ok(Err(HttpResponse::error(
                        500,
                        "PRODUCT_HOST_OUTPUT_POISONED",
                        "output queue lock is poisoned",
                    )));
                };
                outputs.active_binding = Some(binding);
                let queue = Arc::new(SubscriberQueue::default());
                outputs.subscribers.push(Arc::downgrade(&queue));
                Ok(Ok((baseline, result, outputs.next_id, queue)))
            },
            || finish_telemetry(&state, ProductHostOperationKind::Connect),
        )
        .map_err(|_| crate::session::runtime_poisoned())
        .and_then(|response| response);
    let (baseline, result, output_through, queue) = match connection {
        Ok(Ok(connection)) => {
            state.scheduler_wake.notify();
            connection
        }
        Ok(Err(response)) => {
            let _ = write_response(&mut stream, response);
            return;
        }
        Err(error) => {
            let _ = write_response(
                &mut stream,
                HttpResponse::error(500, error.code(), error.diagnostic()),
            );
            return;
        }
    };
    let Ok(completion) = serde_json::to_string(&ProductHostConnectionBaseline {
        result,
        output_through: CanonicalU64::new(output_through),
    }) else {
        let _ = write_response(
            &mut stream,
            HttpResponse::error(
                500,
                "PRODUCT_HOST_RESPONSE_ENCODE",
                "runtime connection result could not be encoded",
            ),
        );
        return;
    };
    if write_sse_headers(&mut stream).is_err() {
        return;
    }
    for batch in baseline {
        let payload = format!("data: {batch}\n\n");
        if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
            return;
        }
    }
    let payload = format!("event: rusty-output-baseline\ndata: {completion}\n\n");
    if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
        return;
    }
    let mut last_write = Instant::now();
    loop {
        if state.shutdown.load(Ordering::Acquire) {
            break;
        }
        let observed_generation = state.output_wake.generation();
        let Some(events) = queue.take() else {
            // This subscriber fell behind. Closing the stream makes the
            // browser reconnect for a fresh baseline.
            break;
        };
        let had_events = !events.is_empty();
        for event in events {
            let payload = match event.name {
                Some(name) => format!("event: {name}\ndata: {}\n\n", event.json),
                None => format!("id: {}\ndata: {}\n\n", event.id, event.json),
            };
            if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
                return;
            }
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

/// Publication state for the runtime's SSE subscribers. Each subscriber owns
/// a bounded live queue; there is no shared history to resume from.
#[derive(Default)]
struct OutputBus {
    /// Sequence of the last published event, sent as its SSE id so a caller
    /// can wait until an operation's outputs have been observed.
    next_id: u64,
    active_binding: Option<crate::ProductHostRuntimeBinding>,
    subscribers: Vec<Weak<SubscriberQueue>>,
}

#[derive(Clone)]
struct OutputEvent {
    id: u64,
    json: Arc<str>,
    /// A named event is not an output batch and carries no sequence id.
    name: Option<&'static str>,
}

#[derive(Default)]
struct SubscriberQueue {
    events: Mutex<VecDeque<OutputEvent>>,
    overflowed: AtomicBool,
}

impl SubscriberQueue {
    fn push(&self, event: &OutputEvent) {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.overflowed.load(Ordering::Acquire) {
            return;
        }
        if events.len() == MAX_SUBSCRIBER_QUEUE_EVENTS {
            events.clear();
            self.overflowed.store(true, Ordering::Release);
            return;
        }
        events.push_back(event.clone());
    }

    /// Takes the queued events, or `None` once the subscriber has overflowed.
    fn take(&self) -> Option<Vec<OutputEvent>> {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.overflowed.load(Ordering::Acquire) {
            return None;
        }
        Some(events.drain(..).collect())
    }

    fn len(&self) -> usize {
        self.events
            .lock()
            .map(|events| events.len())
            .unwrap_or_default()
    }
}

impl OutputBus {
    fn publish(&mut self, batches: Vec<String>) {
        self.subscribers
            .retain(|subscriber| subscriber.strong_count() > 0);
        for json in batches {
            self.next_id += 1;
            let event = OutputEvent {
                id: self.next_id,
                json: json.into(),
                name: None,
            };
            for subscriber in self.subscribers.iter().filter_map(Weak::upgrade) {
                subscriber.push(&event);
            }
        }
    }

    fn publish_ui_reloaded(&mut self) {
        self.subscribers
            .retain(|subscriber| subscriber.strong_count() > 0);
        let event = OutputEvent {
            id: 0,
            json: "{}".into(),
            name: Some("rusty-ui-reloaded"),
        };
        for subscriber in self.subscribers.iter().filter_map(Weak::upgrade) {
            subscriber.push(&event);
        }
    }

    /// The longest unsent backlog among live subscribers.
    fn largest_backlog(&self) -> usize {
        self.subscribers
            .iter()
            .filter_map(Weak::upgrade)
            .map(|subscriber| subscriber.len())
            .max()
            .unwrap_or_default()
    }
}

/// Publishes one operation's outputs to every subscriber. A failure fences
/// the binding, so the next publication must start a complete baseline.
fn push_outputs(
    bus: &Mutex<OutputBus>,
    outputs: Vec<ProductHostRuntimeOutput>,
) -> Result<u64, ProductHostError> {
    let mut bus = bus.lock().map_err(|_| {
        ProductHostError::new(
            "PRODUCT_HOST_OUTPUT_POISONED",
            "output queue lock is poisoned",
        )
    })?;
    match encode_output_batches(bus.active_binding, outputs) {
        Ok((batches, binding)) => {
            bus.active_binding = binding;
            bus.publish(batches);
            Ok(bus.next_id)
        }
        Err(error) => {
            bus.active_binding = None;
            Err(error)
        }
    }
}

fn push_host_outputs<R: ProductHostRuntime>(
    state: &HostState<R>,
    outputs: Vec<ProductHostRuntimeOutput>,
) -> Result<u64, ProductHostError> {
    let changed = !outputs.is_empty();
    let output_through = push_outputs(&state.outputs, outputs)?;
    if changed {
        state.output_wake.notify();
    }
    Ok(output_through)
}

/// Encodes one operation's outputs as SSE batches. A binding opens a
/// baseline that must complete within the same operation; the baseline and
/// each run of incremental outputs become one batch each. Incremental outputs
/// with no active binding are dropped: before a browser attaches, or after a
/// fence, there is no subscriber state to apply them to, and the next
/// connection starts from its own complete baseline. Returns the batches and
/// the binding that is active afterwards.
fn encode_output_batches(
    mut active_binding: Option<crate::ProductHostRuntimeBinding>,
    outputs: Vec<ProductHostRuntimeOutput>,
) -> Result<(Vec<String>, Option<crate::ProductHostRuntimeBinding>), ProductHostError> {
    let mut batches = Vec::new();
    let mut incremental = Vec::new();
    let mut baseline: Option<(
        crate::ProductHostRuntimeBinding,
        Vec<ProductHostRuntimeOutput>,
    )> = None;
    for output in outputs {
        if let Some(binding) = output.binding_marker() {
            if baseline.is_some() {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_OUTPUT_BASELINE",
                    "a new binding arrived before the previous baseline completed",
                ));
            }
            if !incremental.is_empty() {
                batches.push(encode_output_batch(&std::mem::take(&mut incremental))?);
            }
            // A runtime that begins a replacement baseline owns subsequent
            // publication. Fence the previous binding immediately so a
            // rejected replacement cannot label later incrementals with the
            // stale runtime identity.
            active_binding = None;
            baseline = Some((binding, vec![output]));
            continue;
        }
        if let Some(binding) = output.complete_baseline_marker() {
            let Some((pending, members)) = baseline.take() else {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_OUTPUT_BASELINE",
                    "a baseline completion arrived without its binding",
                ));
            };
            if pending != binding {
                return Err(ProductHostError::new(
                    "PRODUCT_HOST_OUTPUT_BASELINE",
                    "a baseline completion does not match its binding",
                ));
            }
            batches.push(encode_output_batch(&members)?);
            active_binding = Some(binding);
            continue;
        }
        if let Some((_, members)) = &mut baseline {
            members.push(output);
            continue;
        }
        if active_binding.is_some() {
            incremental.push(output);
        }
    }
    if baseline.is_some() {
        return Err(ProductHostError::new(
            "PRODUCT_HOST_OUTPUT_BASELINE",
            "a baseline did not complete within its publication",
        ));
    }
    if !incremental.is_empty() {
        batches.push(encode_output_batch(&incremental)?);
    }
    Ok((batches, active_binding))
}

/// One SSE `data` event: the outputs published together, in order.
fn encode_output_batch(outputs: &[ProductHostRuntimeOutput]) -> Result<String, ProductHostError> {
    serde_json::to_string(outputs)
        .map_err(|error| ProductHostError::new("PRODUCT_HOST_OUTPUT_ENCODE", error.to_string()))
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

    /// The runtime routes this host answers. A shipped window-mode product
    /// serves every route except the live-debug ones, so a new route joins
    /// one of these lists deliberately (see the crate doc).
    const SHIPPED_ROUTES: &[&str] = &[
        "admit-demand-step",
        "admit-external-step",
        "advance-realtime",
        "browser-diagnostics",
        "control/claim",
        "control/release",
        "control/replace",
        "frames",
        "frames/capture",
        "input",
        "lifecycle/pause",
        "lifecycle/report-fault",
        "lifecycle/restart",
        "lifecycle/resume",
        "lifecycle/shutdown",
        "lifecycle/start",
        "outputs/fresh",
        "timeline-completion",
    ];
    /// Answered only with `--live-debug`.
    const LIVE_DEBUG_ROUTES: &[&str] = &["debug/catalog", "debug/execute", "diagnostics/read"];

    #[test]
    fn every_runtime_route_is_listed_as_shipped_or_live_debug() {
        let mut found = std::collections::BTreeSet::new();
        for source in [include_str!("host.rs"), include_str!("frames.rs")] {
            let tests = source.find("#[cfg(test)]\nmod tests {");
            let code = &source[..tests.unwrap_or(source.len())];
            for literal in code.split('"').skip(1).step_by(2) {
                if let Some(route) = literal.strip_prefix(crate::PRODUCT_HOST_RUNTIME_BASE_PATH) {
                    if !route.is_empty() {
                        found.insert(route.to_owned());
                    }
                }
            }
        }
        let listed: std::collections::BTreeSet<String> = SHIPPED_ROUTES
            .iter()
            .chain(LIVE_DEBUG_ROUTES)
            .map(|route| (*route).to_owned())
            .collect();
        assert_eq!(found, listed, "list a new route as shipped or live-debug");
    }

    #[test]
    fn a_failed_content_reload_keeps_the_served_ui_and_tells_no_page() {
        fn bundle(body: &[u8]) -> ProductHostBundle {
            ProductHostBundle::new(vec![crate::ProductHostBundleEntry::new(
                "index.html",
                "text/html; charset=utf-8",
                body.to_vec(),
            )
            .unwrap()])
            .unwrap()
        }
        let served = Arc::new(RwLock::new(bundle(b"old UI")));
        let page = Arc::new(SubscriberQueue::default());
        let outputs = Arc::new(Mutex::new(OutputBus {
            subscribers: vec![Arc::downgrade(&page)],
            ..OutputBus::default()
        }));
        let reload = |content: Result<(), ProductHostRuntimeError>| ProductHostAssetReload {
            bundle: Arc::clone(&served),
            content: Arc::new(move || content.clone()),
            outputs: Arc::clone(&outputs),
            output_wake: Arc::new(OutputWake::default()),
        };
        let failed = reload(Err(ProductHostRuntimeError::new(
            "CONTENT_BUNDLE_INDEX",
            "invalid index",
        )));
        assert!(failed.reload(bundle(b"new UI")).is_err());
        assert_eq!(served.read().unwrap().get("/").unwrap().bytes(), b"old UI");
        assert!(page.take().unwrap().is_empty());
        reload(Ok(())).reload(bundle(b"new UI")).unwrap();
        assert_eq!(served.read().unwrap().get("/").unwrap().bytes(), b"new UI");
        let events = page.take().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, Some("rusty-ui-reloaded"));
        // Not an output: the output sequence does not advance.
        assert_eq!(outputs.lock().unwrap().next_id, 0);
    }

    #[test]
    fn response_delivery_certainty_distinguishes_receipts_mailbox_and_observations() {
        let settled =
            HttpResponse::bytes(200, "application/json", Vec::new()).with_output_through(7);
        assert!(matches!(
            settled.delivery_certainty,
            Some(ResponseDeliveryCertainty::Settled)
        ));

        let queued = HttpResponse::bytes(200, "application/json", Vec::new()).with_queued_input();
        assert!(matches!(
            queued.delivery_certainty,
            Some(ResponseDeliveryCertainty::QueuedInput)
        ));

        let observation =
            HttpResponse::bytes(200, "application/json", Vec::new()).with_observation();
        assert!(matches!(
            observation.delivery_certainty,
            Some(ResponseDeliveryCertainty::Observation)
        ));

        let unclassified =
            HttpResponse::bytes(200, "application/json", Vec::new()).with_resync_required();
        assert!(unclassified.delivery_certainty.is_none());
    }

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

    fn blocking_runtime_error() -> crate::ProductHostRuntimeError {
        crate::ProductHostRuntimeError::new("TEST_RUNTIME", "test runtime operation")
    }

    impl crate::ProductHostRuntime for BlockingRealtimeRuntime {
        fn realtime_schedule_state(&self) -> crate::ProductHostRuntimeScheduleState {
            crate::ProductHostRuntimeScheduleState::Running
        }

        fn realtime_schedule_interval(&self) -> Option<Duration> {
            Some(Duration::from_millis(1))
        }

        fn lifecycle(
            &mut self,
            _operation: crate::ProductHostLifecycleOperation,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn input(
            &mut self,
            _batch: crate::ProductHostInputBatch,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostInputResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn advance_realtime(
            &mut self,
            _observed_time_ns: CanonicalU64,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_demand_step(
            &mut self,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_external_step(
            &mut self,
            _step: CanonicalU64,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn complete_timeline(
            &mut self,
            _completion: crate::ProductHostTimelineCompletion,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostTimelineCompletionResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }
    }

    fn binding() -> crate::ProductHostRuntimeBinding {
        crate::ProductHostRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(2),
        }
    }

    #[test]
    fn input_mailbox_overflow_clears_prefix_and_marks_resync() {
        let mailbox = HostInputMailbox::default();
        for _ in 0..MAX_HOST_INPUT_BATCHES {
            assert!(mailbox.enqueue(ProductHostInputBatch::new(Vec::new()), Some(0)));
        }
        assert_eq!(mailbox.len(), MAX_HOST_INPUT_BATCHES);
        assert!(!mailbox.enqueue(ProductHostInputBatch::new(Vec::new()), Some(0)));

        let (batches, overflowed) = mailbox.drain();
        assert!(
            batches.is_empty(),
            "overflow must not retain a stale prefix"
        );
        assert!(
            overflowed,
            "scheduler must receive an explicit resync marker"
        );

        assert!(mailbox.enqueue(ProductHostInputBatch::new(Vec::new()), Some(0)));
        let (batches, overflowed) = mailbox.drain();
        assert_eq!(batches.len(), 1);
        assert!(!overflowed);
    }

    fn subscribed_bus() -> (Mutex<OutputBus>, Arc<SubscriberQueue>) {
        let queue = Arc::new(SubscriberQueue::default());
        let bus = OutputBus {
            active_binding: Some(binding()),
            subscribers: vec![Arc::downgrade(&queue)],
            ..OutputBus::default()
        };
        (Mutex::new(bus), queue)
    }

    fn batch_json(event: &OutputEvent) -> Value {
        serde_json::from_str(&event.json).unwrap()
    }

    #[test]
    fn in_place_rebind_group_publishes_as_one_batch() {
        let (bus, queue) = subscribed_bus();
        let paused = crate::ProductHostRuntimeBinding {
            control_revision: CanonicalU64::new(3),
            ..binding()
        };
        // A same-incarnation fence: binding, the callback's own outputs, and
        // completion, without any world snapshot between them.
        push_outputs(
            &bus,
            vec![
                ProductHostRuntimeOutput::binding(paused, CanonicalU64::new(5)),
                ProductHostRuntimeOutput::test_value(serde_json::json!({})),
                ProductHostRuntimeOutput::complete_baseline(paused),
            ],
        )
        .expect("in-place rebind publishes");
        assert_eq!(bus.lock().unwrap().active_binding, Some(paused));
        let events = queue.take().unwrap();
        assert_eq!(events.len(), 1);
        let value = batch_json(&events[0]);
        assert_eq!(value[0]["kind"], "binding");
        assert_eq!(value[0]["runtime"]["controlRevision"], "3");
        assert_eq!(value[1]["kind"], "ui-projection");
        assert_eq!(value.as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn one_receipt_encodes_as_one_ordered_output_batch() {
        let (bus, queue) = subscribed_bus();
        let output_through = push_outputs(
            &bus,
            vec![
                ProductHostRuntimeOutput::runtime_readout(crate::ProductHostRuntimeReadout::new(
                    binding(),
                    crate::ProductHostRuntimeMode::Realtime,
                    crate::ProductHostRuntimeState::Running,
                )),
                ProductHostRuntimeOutput::test_value(serde_json::json!({})),
            ],
        )
        .expect("receipt batch publishes");
        let events = queue.take().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, output_through);
        let value = batch_json(&events[0]);
        assert_eq!(value.as_array().map(Vec::len), Some(2));
        assert_eq!(value[0]["kind"], "runtime-readout");
        assert_eq!(value[1]["kind"], "ui-projection");
    }

    #[test]
    fn sixty_hertz_receipts_deliver_one_event_per_receipt_in_order() {
        let (bus, queue) = subscribed_bus();
        for tick in 0..60 {
            push_outputs(&bus, representative_realtime_receipt(tick)).unwrap();
        }
        let events = queue.take().unwrap();
        assert_eq!(events.len(), 60);
        for (tick, event) in events.iter().enumerate() {
            let value = batch_json(event);
            let kinds: Vec<_> = value
                .as_array()
                .unwrap()
                .iter()
                .map(|output| output["kind"].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(kinds, ["ui-projection", "ui-projection", "runtime-readout"]);
            assert_eq!(
                value[0]["envelope"]["value"]["tick"],
                serde_json::json!(tick)
            );
        }
    }

    #[test]
    fn subscribers_share_one_encoding_and_see_only_later_publications() {
        let (bus, early) = subscribed_bus();
        push_outputs(
            &bus,
            vec![ProductHostRuntimeOutput::test_value(
                serde_json::json!({"n": 1}),
            )],
        )
        .unwrap();
        let late = Arc::new(SubscriberQueue::default());
        bus.lock().unwrap().subscribers.push(Arc::downgrade(&late));
        push_outputs(
            &bus,
            vec![ProductHostRuntimeOutput::test_value(
                serde_json::json!({"n": 2}),
            )],
        )
        .unwrap();
        let early = early.take().unwrap();
        let late = late.take().unwrap();
        assert_eq!(early.len(), 2);
        assert_eq!(late.len(), 1);
        assert!(Arc::ptr_eq(&early[1].json, &late[0].json));
        assert_eq!(late[0].id, 2);
    }

    #[test]
    fn a_subscriber_that_falls_behind_is_closed_and_a_dropped_one_is_pruned() {
        let (bus, slow) = subscribed_bus();
        let gone = Arc::new(SubscriberQueue::default());
        bus.lock().unwrap().subscribers.push(Arc::downgrade(&gone));
        drop(gone);
        for tick in 0..=MAX_SUBSCRIBER_QUEUE_EVENTS as u64 {
            push_outputs(&bus, representative_realtime_receipt(tick)).unwrap();
        }
        assert!(
            slow.take().is_none(),
            "an overflowed subscriber must reconnect fresh"
        );
        assert_eq!(bus.lock().unwrap().subscribers.len(), 1);
    }

    #[test]
    fn a_large_output_is_one_event() {
        let (bus, queue) = subscribed_bus();
        let payload = "x".repeat(4 * 1024 * 1024);
        push_outputs(
            &bus,
            vec![ProductHostRuntimeOutput::test_value(
                serde_json::json!({ "payload": payload }),
            )],
        )
        .unwrap();
        let events = queue.take().unwrap();
        assert_eq!(events.len(), 1);
        let carried = batch_json(&events[0])[0]["envelope"]["value"]["payload"]
            .as_str()
            .map(str::len);
        assert_eq!(carried, Some(payload.len()));
    }

    #[test]
    fn a_rejected_publication_sends_nothing_and_fences_the_binding() {
        let (bus, queue) = subscribed_bus();
        let replacement = crate::ProductHostRuntimeBinding {
            generation: CanonicalU64::new(binding().generation.get() + 1),
            ..binding()
        };
        let error = push_outputs(
            &bus,
            vec![
                ProductHostRuntimeOutput::test_value(serde_json::json!({})),
                ProductHostRuntimeOutput::binding(replacement, CanonicalU64::new(0)),
                ProductHostRuntimeOutput::test_value(serde_json::json!({})),
            ],
        )
        .expect_err("a baseline must complete within its publication");
        assert_eq!(error.code(), "PRODUCT_HOST_OUTPUT_BASELINE");
        assert!(queue.take().unwrap().is_empty());
        assert_eq!(bus.lock().unwrap().active_binding, None);
        push_outputs(
            &bus,
            vec![ProductHostRuntimeOutput::test_value(serde_json::json!({}))],
        )
        .expect("incrementals after a fence are dropped, not rejected");
        assert!(queue.take().unwrap().is_empty());
        assert_eq!(bus.lock().unwrap().active_binding, None);
    }

    #[test]
    fn telemetry_snapshot_is_bounded_and_keeps_subsecond_rates() {
        let mut telemetry = HostTelemetry::default();
        telemetry.begin(ProductHostOperationKind::AdvanceRealtime, 1_000_000);
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
                output_queue_items: MAX_SUBSCRIBER_QUEUE_EVENTS,
                output_binding_active: true,
            },
        );
        assert_eq!(
            snapshot.in_flight_operation,
            Some(ProductHostOperationKind::AdvanceRealtime)
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
        assert_eq!(wire["outputQueueItems"], MAX_SUBSCRIBER_QUEUE_EVENTS);
        assert_eq!(wire["runtimeProgressRateMillihertz"], "500");
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < 4 * 1024);
    }

    #[test]
    fn update_attribution_retains_a_long_window_and_lifetime_slowest_sample() {
        let mut telemetry = HostTelemetry::default();
        let sample = |duration_us| ProductHostUpdateAttribution {
            callback_duration_us: CanonicalU64::new(duration_us),
            ..ProductHostUpdateAttribution::default()
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
        let accepted = ProductHostInputResult::with_progress(
            2,
            2,
            0,
            Some(CanonicalU64::new(4)),
            Some(CanonicalU64::new(4)),
            CanonicalU64::new(5),
            binding(),
            crate::ProductHostRuntimeReadout::new(
                binding(),
                crate::ProductHostRuntimeMode::Realtime,
                crate::ProductHostRuntimeState::Running,
            ),
        )
        .unwrap();
        let accepted_wire =
            serde_json::to_value(ProductHostRuntimeOutput::runtime_input_result(accepted)).unwrap();
        assert_eq!(accepted_wire["kind"], "runtime-input-result");
        assert_eq!(accepted_wire["result"]["acceptedThrough"], "4");
        assert_eq!(accepted_wire["result"]["consumedThrough"], "4");
        assert_eq!(accepted_wire["result"]["nextInputSequence"], "5");
        assert_eq!(accepted_wire["result"]["disposition"], "accepted");

        let stale = ProductHostInputResult::with_progress(
            2,
            1,
            1,
            Some(CanonicalU64::new(6)),
            Some(CanonicalU64::new(7)),
            CanonicalU64::new(8),
            binding(),
            crate::ProductHostRuntimeReadout::new(
                binding(),
                crate::ProductHostRuntimeMode::Realtime,
                crate::ProductHostRuntimeState::Running,
            ),
        )
        .unwrap();
        let stale_wire =
            serde_json::to_value(ProductHostRuntimeOutput::runtime_input_result(stale)).unwrap();
        assert_eq!(stale_wire["result"]["accepted"], false);
        assert_eq!(stale_wire["result"]["disposition"], "rejected-recoverable");
        assert_eq!(stale_wire["result"]["acceptedThrough"], "6");
        assert_eq!(stale_wire["result"]["consumedThrough"], "7");

        let overflow = ProductHostInputResult::mailbox_full(2).unwrap();
        let overflow_wire =
            serde_json::to_value(ProductHostRuntimeOutput::runtime_input_result(overflow)).unwrap();
        assert_eq!(overflow_wire["result"]["accepted"], false);
        assert_eq!(overflow_wire["result"]["disposition"], "resync-required");
    }

    /// A realtime runtime whose playtest time is held: the scheduler idles, so
    /// only debug commands advance it. It records the order of its calls.
    struct HeldRealtimeRuntime(Arc<Mutex<Vec<String>>>);

    impl crate::ProductHostRuntime for HeldRealtimeRuntime {
        fn realtime_schedule_state(&self) -> crate::ProductHostRuntimeScheduleState {
            crate::ProductHostRuntimeScheduleState::Paused
        }

        fn realtime_schedule_interval(&self) -> Option<Duration> {
            Some(Duration::from_millis(1))
        }

        fn lifecycle(
            &mut self,
            _operation: crate::ProductHostLifecycleOperation,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn input(
            &mut self,
            batch: crate::ProductHostInputBatch,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostInputResult>,
            crate::ProductHostRuntimeError,
        > {
            self.0
                .lock()
                .unwrap()
                .push(format!("input {}", batch.events().len()));
            Err(blocking_runtime_error())
        }

        fn execute_debug(
            &mut self,
            command: &str,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostDebugResult>,
            crate::ProductHostRuntimeError,
        > {
            self.0.lock().unwrap().push(command.to_owned());
            crate::ProductHostRuntimeReceipt::new(
                crate::ProductHostDebugResult::new(true, String::new()),
                Vec::new(),
            )
            .map_err(|error| crate::ProductHostRuntimeError::new(error.code(), error.detail()))
        }

        fn advance_realtime(
            &mut self,
            _observed_time_ns: CanonicalU64,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_demand_step(
            &mut self,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn admit_external_step(
            &mut self,
            _step: CanonicalU64,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostOperationResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }

        fn complete_timeline(
            &mut self,
            _completion: crate::ProductHostTimelineCompletion,
        ) -> Result<
            crate::ProductHostRuntimeReceipt<crate::ProductHostTimelineCompletionResult>,
            crate::ProductHostRuntimeError,
        > {
            Err(blocking_runtime_error())
        }
    }

    #[test]
    fn a_debug_command_takes_the_input_queued_before_it() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let state = HostState {
            bundle: Arc::new(RwLock::new(
                ProductHostBundle::new(vec![crate::ProductHostBundleEntry::new(
                    "index.html",
                    "text/html; charset=utf-8",
                    Vec::new(),
                )
                .unwrap()])
                .unwrap(),
            )),
            runtime: Arc::new(ProductHostOperationOwner::new(HeldRealtimeRuntime(
                Arc::clone(&calls),
            ))),
            input_mailbox: Arc::new(HostInputMailbox::default()),
            telemetry: Arc::new(Mutex::new(HostTelemetry::default())),
            realtime_scheduler_enabled: true,
            outputs: Arc::new(Mutex::new(OutputBus::default())),
            output_wake: Arc::new(OutputWake::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            scheduler_wake: Arc::new(SchedulerWake::default()),
            bind_host: Ipv4Addr::LOCALHOST,
            expected_port: 0,
            live_debug_enabled: true,
            diagnostics: ProductHostLog::new(Default::default()).unwrap(),
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
            published_readout: Mutex::new(None),
            frames: None,
            capture: None,
        };
        // Held time: nothing ticks, so the key waits in the mailbox.
        let queued = invoke_input(&state, br#"{"batch":[{"runtime":{"instanceId":"41","generation":"1","controlRevision":"1"},"sequence":"14","context":"gameplay.default","fact":{"kind":"key","code":"key-w","edge":"pressed"}}]}"#);
        assert_eq!(queued.status, 200);
        assert_eq!(state.input_mailbox.len(), 1);

        invoke_debug_execute(&state, b"engine.time.advance 500");
        assert_eq!(state.input_mailbox.len(), 0);
        assert_eq!(
            *calls.lock().unwrap(),
            ["input 1", "engine.time.advance 500"],
            "the steps the command runs must see the key pressed before it"
        );
    }

    #[test]
    fn realtime_input_admission_does_not_wait_for_runtime_owner() {
        let runtime = Arc::new(ProductHostOperationOwner::new(BlockingRealtimeRuntime));
        let state = Arc::new(HostState {
            bundle: Arc::new(RwLock::new(
                ProductHostBundle::new(vec![crate::ProductHostBundleEntry::new(
                    "index.html",
                    "text/html; charset=utf-8",
                    Vec::new(),
                )
                .unwrap()])
                .unwrap(),
            )),
            runtime: Arc::clone(&runtime),
            input_mailbox: Arc::new(HostInputMailbox::default()),
            telemetry: Arc::new(Mutex::new(HostTelemetry::default())),
            realtime_scheduler_enabled: true,
            outputs: Arc::new(Mutex::new(OutputBus::default())),
            output_wake: Arc::new(OutputWake::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            scheduler_wake: Arc::new(SchedulerWake::default()),
            bind_host: Ipv4Addr::LOCALHOST,
            expected_port: 0,
            live_debug_enabled: false,
            diagnostics: ProductHostLog::new(Default::default()).unwrap(),
            connections: AtomicUsize::new(0),
            subscribers: AtomicUsize::new(0),
            published_readout: Mutex::new(None),
            frames: None,
            capture: None,
        });
        let (held, held_ready) = std::sync::mpsc::channel();
        let (release, release_owner) = std::sync::mpsc::channel();
        let locked_runtime = Arc::clone(&runtime);
        let owner_thread = thread::spawn(move || {
            locked_runtime
                .session()
                .with_locked(|_| {
                    held.send(()).expect("owner lock marker");
                    release_owner
                        .recv_timeout(Duration::from_secs(1))
                        .expect("owner lock release");
                })
                .map_err(|_| crate::session::runtime_poisoned())
                .expect("owner lock fixture");
        });
        held_ready
            .recv_timeout(Duration::from_secs(1))
            .expect("runtime owner was locked");

        let (response, response_ready) = std::sync::mpsc::channel();
        let input_state = Arc::clone(&state);
        let input_thread = thread::spawn(move || {
            response
                .send(invoke_input(&input_state, br#"{"batch":[{"runtime":{"instanceId":"41","generation":"1","controlRevision":"1"},"sequence":"14","context":"gameplay.default","fact":{"kind":"key","code":"digit-1","edge":"pressed"}},{"runtime":{"instanceId":"41","generation":"1","controlRevision":"1"},"sequence":"15","context":"gameplay.default","fact":{"kind":"key","code":"digit-1","edge":"released"}}]}"#))
                .expect("input response");
        });
        let input_response = response_ready
            .recv_timeout(Duration::from_millis(100))
            .expect("input enqueue must not wait for a slow product update");
        assert_eq!(input_response.status, 200);
        assert!(matches!(
            input_response.commit_disposition,
            Some(CommitDisposition::Committed)
        ));
        assert_eq!(input_response.output_through, None);
        let body: serde_json::Value = serde_json::from_slice(&input_response.body).unwrap();
        assert_eq!(body["accepted"], true);
        assert_eq!(body["acceptedCount"], 2);
        assert_eq!(state.input_mailbox.len(), 1);

        release.send(()).expect("release runtime owner");
        input_thread.join().expect("input worker");
        owner_thread.join().expect("owner worker");
    }

    fn representative_realtime_receipt(tick: u64) -> Vec<ProductHostRuntimeOutput> {
        let runtime = binding();
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
            ProductHostRuntimeOutput::test_value(serde_json::json!({"tick": tick})),
            ProductHostRuntimeOutput::ui_projection(&ui_projection),
            ProductHostRuntimeOutput::runtime_readout(
                crate::ProductHostRuntimeReadout::new(
                    runtime,
                    crate::ProductHostRuntimeMode::Realtime,
                    crate::ProductHostRuntimeState::Running,
                )
                .with_counters(tick + 1, 0, 0, 0)
                .with_clock(None, Some(tick + 1)),
            ),
        ]
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
            "PRODUCT_HOST_SOCKET",
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
                    "PRODUCT_HOST_REQUEST_EOF",
                    "request ended before headers",
                ));
            }
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > MAX_REQUEST_HEADER_BYTES + MAX_REQUEST_BODY_BYTES {
                    return Err(HttpResponse::error(
                        413,
                        "PRODUCT_HOST_REQUEST_BOUNDS",
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
                        "PRODUCT_HOST_HEADER_BOUNDS",
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
                    "PRODUCT_HOST_REQUEST_TIMEOUT",
                    "request header timeout",
                ));
            }
            Err(_) => {
                return Err(HttpResponse::error(
                    400,
                    "PRODUCT_HOST_REQUEST_READ",
                    "could not read request",
                ));
            }
        }
    }
    let head = std::str::from_utf8(&bytes[..header_end - 4]).map_err(|_| {
        HttpResponse::error(
            400,
            "PRODUCT_HOST_HEADER_UTF8",
            "request headers must be ASCII",
        )
    })?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| {
        HttpResponse::error(400, "PRODUCT_HOST_REQUEST_LINE", "request line is required")
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
            "PRODUCT_HOST_REQUEST_LINE",
            "request line is not admitted",
        ));
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(HttpResponse::error(
                400,
                "PRODUCT_HOST_HEADER",
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
                "PRODUCT_HOST_HEADER",
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
            "PRODUCT_HOST_CONNECTION",
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
        HttpResponse::error(
            413,
            "PRODUCT_HOST_BODY_BOUNDS",
            "body length is not admitted",
        )
    })?;
    let mut body = bytes[header_end..].to_vec();
    if body.len() > content_length {
        return Err(HttpResponse::error(
            400,
            "PRODUCT_HOST_BODY_LENGTH",
            "request has trailing bytes",
        ));
    }
    while body.len() < content_length {
        let mut buffer = [0_u8; 1024];
        match stream.read(&mut buffer) {
            Ok(0) => {
                return Err(HttpResponse::error(
                    400,
                    "PRODUCT_HOST_BODY_EOF",
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
                    "PRODUCT_HOST_BODY_TIMEOUT",
                    "request body timeout",
                ));
            }
            Err(_) => {
                return Err(HttpResponse::error(
                    400,
                    "PRODUCT_HOST_BODY_READ",
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
        && !path.contains('#')
        && !path.contains("//")
        && !path.split('/').any(|part| part == "." || part == "..")
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'-' | b'_' | b'?' | b'&' | b'=' | b'%')
        })
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
    body: Arc<[u8]>,
    output_through: Option<u64>,
    commit_disposition: Option<CommitDisposition>,
    delivery_certainty: Option<ResponseDeliveryCertainty>,
    /// A capture's drawn cameras, as JSON.
    cameras: Option<String>,
}

/// Preserve the runtime's mutation certainty even on a typed rejection.
/// `ResyncRequired` means the runtime result was committed, but the caller
/// must use its existing binding/readout identity and `/outputs/fresh` rather
/// than replaying the route request.
#[derive(Clone, Copy)]
enum CommitDisposition {
    NotApplied,
    Unknown,
    Committed,
    ResyncRequired,
}

/// What the host knows was admitted when its HTTP response cannot be written.
/// This is intentionally independent from the public commit header, which
/// also describes recovery instructions for non-settled route responses.
#[derive(Clone, Copy)]
enum ResponseDeliveryCertainty {
    Settled,
    QueuedInput,
    Observation,
}

impl ResponseDeliveryCertainty {
    const fn as_field(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::QueuedInput => "queued-input",
            Self::Observation => "observation",
        }
    }
}

impl CommitDisposition {
    const fn as_header(self) -> &'static str {
        match self {
            Self::NotApplied => "not-applied",
            Self::Unknown => "unknown",
            Self::Committed => "committed",
            Self::ResyncRequired => "resync-required",
        }
    }
}

impl HttpResponse {
    fn bytes(status: u16, content_type: impl Into<String>, body: impl Into<Arc<[u8]>>) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body: body.into(),
            output_through: None,
            commit_disposition: None,
            delivery_certainty: None,
            cameras: None,
        }
    }

    /// Only a failure rejected before mutation is known not to have applied.
    fn with_runtime_error(mut self, disposition: crate::ProductHostFaultDisposition) -> Self {
        self.commit_disposition = Some(match disposition {
            crate::ProductHostFaultDisposition::RejectedRecoverable => {
                CommitDisposition::NotApplied
            }
            _ => CommitDisposition::Unknown,
        });
        self
    }

    fn with_output_through(mut self, output_through: u64) -> Self {
        self.output_through = Some(output_through);
        self.commit_disposition = Some(CommitDisposition::Committed);
        self.delivery_certainty = Some(ResponseDeliveryCertainty::Settled);
        self
    }

    fn with_queued_input(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::Committed);
        self.delivery_certainty = Some(ResponseDeliveryCertainty::QueuedInput);
        self
    }

    fn with_observation(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::Committed);
        self.delivery_certainty = Some(ResponseDeliveryCertainty::Observation);
        self
    }

    fn with_resync_required(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::ResyncRequired);
        self
    }

    fn with_settled_resync_required(mut self) -> Self {
        self.commit_disposition = Some(CommitDisposition::ResyncRequired);
        self.delivery_certainty = Some(ResponseDeliveryCertainty::Settled);
        self
    }

    fn text(status: u16, text: String) -> Self {
        Self::bytes(status, "text/plain; charset=utf-8", text.into_bytes())
    }

    fn error(status: u16, code: &str, detail: &str) -> Self {
        let body = ProductHostErrorResponse {
            accepted: false,
            error: ProductHostErrorBody {
                code: code.to_owned(),
                diagnostic: detail.to_owned(),
            },
        };
        let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
        Self::bytes(status, "application/json", bytes)
    }
}

fn debug_text_error(status: u16, detail: &str) -> HttpResponse {
    HttpResponse::text(status, detail.to_owned())
}

fn write_response(stream: &mut TcpStream, response: HttpResponse) -> io::Result<()> {
    stream.set_write_timeout(Some(SOCKET_TIMEOUT))?;
    let reason = match response.status {
        200 => "OK",
        204 => "No Content",
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
    if let Some(cameras) = &response.cameras {
        write!(stream, "X-Rusty-Frame-Cameras: {cameras}\r\n")?;
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
    stream.write_all(b"Cache-Control: no-store\r\n")?;
    stream.write_all(b"X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n")?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn write_sse_headers(stream: &mut TcpStream) -> io::Result<()> {
    stream.set_write_timeout(Some(SOCKET_TIMEOUT))?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: keep-alive\r\n\r\n")?;
    stream.flush()
}

/// The body of every host error response.
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductHostErrorResponse {
    #[ts(type = "false")]
    accepted: bool,
    error: ProductHostErrorBody,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductHostErrorBody {
    code: String,
    diagnostic: String,
}

/// The body of a route that takes no arguments: `{}`.
#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProductHostEmptyRequest {}

/// `diagnostics/read`: diagnostics after a cursor, or the retained ones.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostDiagnosticsReadRequest {
    #[serde(default)]
    #[ts(optional)]
    after: Option<CanonicalU64>,
}

/// The `diagnostics/read` answer: retained diagnostics and host telemetry.
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductHostDiagnosticsReadResponse {
    #[serde(flatten)]
    batch: crate::ProductHostLogBatch,
    telemetry: ProductHostTelemetrySnapshot,
}

/// A lifecycle route's body. `runtime` names the binding the operation is
/// meant for; without it, the current one.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostLifecycleRequest {
    #[serde(default)]
    #[ts(optional)]
    runtime: Option<crate::ProductHostRuntimeBinding>,
}

/// `control/replace`: advance the input control fence of `runtime`.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostControlRequest {
    runtime: crate::ProductHostRuntimeBinding,
}

/// `control/claim`: a harness takes input from `runtime`'s owner under a
/// fresh binding, labelled for any attached page, for `leaseMs` after its
/// last input.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostControlClaimRequest {
    runtime: crate::ProductHostRuntimeBinding,
    label: String,
    lease_ms: CanonicalU64,
}

/// `input`: one ordered input batch.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostInputRequest {
    #[ts(as = "Vec<runtime_input::RuntimeInputWireEvent>")]
    batch: Vec<Value>,
}

/// `advance-realtime`: the page's monotonic clock, in nanoseconds.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostRealtimeRequest {
    observed_time_ns: CanonicalU64,
}

/// `admit-external-step`: the step an external clock admits.
#[derive(Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProductHostExternalRequest {
    step: CanonicalU64,
}

/// The `rusty-output-baseline` event that ends a connection's baseline.
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProductHostConnectionBaseline {
    #[serde(flatten)]
    result: ProductHostOperationResult,
    /// The output sequence at the baseline, so a caller can wait for a
    /// later operation's outputs.
    output_through: CanonicalU64,
}

fn decode_empty(body: &[u8]) -> Result<(), HttpResponse> {
    decode_json::<ProductHostEmptyRequest>(body).map(|_| ())
}

fn decode_json<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, HttpResponse> {
    if body.len() > MAX_REQUEST_BODY_BYTES {
        return Err(HttpResponse::error(
            413,
            "PRODUCT_HOST_BODY_BOUNDS",
            "request body exceeds host bound",
        ));
    }
    serde_json::from_slice(body).map_err(|_| {
        HttpResponse::error(
            400,
            "PRODUCT_HOST_JSON",
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
            "PRODUCT_HOST_RESPONSE_ENCODE",
            "response could not be encoded",
        ),
    }
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
