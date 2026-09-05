use std::{
    collections::VecDeque,
    env, fs,
    io::{Read, Write},
    net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use csharp_product_runtime::{
    product_host_runtime_identity, CsharpProductContent, CsharpProductRuntime,
    CsharpProductRuntimeConfig,
};
use product_dev_host::{
    advance_realtime_with_input_and_publish, read_worker_frame, write_worker_frame, CanonicalU64,
    ProductDevAnimationFeedback, ProductDevAnimationFeedbackResult, ProductDevAudioFeedback,
    ProductDevAudioFeedbackResult, ProductDevBundle, ProductDevBundleEntry,
    ProductDevControlOperation, ProductDevDebugCatalog, ProductDevDebugResult,
    ProductDevGhostPlateFeedback, ProductDevGhostPlateFeedbackResult, ProductDevHost,
    ProductDevHostConfig, ProductDevInputBatch, ProductDevInputResult,
    ProductDevLifecycleOperation, ProductDevLog, ProductDevOperationOwner,
    ProductDevOperationResult, ProductDevRendererDiagnosticsFeedback,
    ProductDevRendererDiagnosticsFeedbackResult, ProductDevRendererResource, ProductDevRuntime,
    ProductDevRuntimeBinding, ProductDevRuntimeError, ProductDevRuntimeOutput,
    ProductDevRuntimeReceipt, ProductDevRuntimeScheduleState, ProductDevTimelineCompletion,
    ProductDevTimelineCompletionResult, ProductDevWorkerBundle, ProductDevWorkerBundleEntry,
    ProductDevWorkerControlOperation, ProductDevWorkerDiagnostic, ProductDevWorkerEvent,
    ProductDevWorkerFault, ProductDevWorkerFeedbackOperation, ProductDevWorkerLifecycleOperation,
    ProductDevWorkerOutputBatch, ProductDevWorkerPublication, ProductDevWorkerRequest,
    ProductDevWorkerResponse, ProductDevWorkerUpdateOperation, ProductDevWorkerUpdateTelemetry,
    RunningProductDevHost,
};
use runtime_input::{
    CompiledInputMappings, ControllerAxis, ControllerButton, DirectInputIntentDescriptor,
    InputAxis, InputContext, InputEdge, IntentValueKind, KeyboardControl, PointerButton,
    RuntimeInputMapping, RuntimeInputTrigger,
};
use runtime_lifecycle::RuntimeInstanceId;

mod product_bundle;
use product_bundle::ProductBundle;

const MAX_PHYSICAL_MAPPINGS: usize = 256;
const MAX_MAPPING_CHORD_CONTROLS: usize = 8;
static NEXT_DIRECT_RUNTIME_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);

const PHYSICAL_MAPPING_USAGE: &str = "--physical-mapping <mapping-id>=<intent-id>:<trigger>\n\
  key:<keyboard-control>:<held|pressed|released>[:context=<identity>][:chord=<keyboard-control>+...]\n\
  pointer-button:<primary|secondary|middle>:<held|pressed|released>[:context=<identity>]\n\
  pointer-axis:<x|y>[:context=<identity>]\n\
  wheel:<x|y>[:context=<identity>]\n\
  controller-button:<button-0..button-15>:<held|pressed|released>[:context=<identity>]\n\
  controller-axis:<axis-0..axis-3>[:context=<identity>]\n\
keyboard controls: key-a..key-z, digit-0..digit-9, space, enter, escape, shift-left,\n\
  shift-right, control-left, control-right, alt-left, alt-right";

fn main() -> Result<(), String> {
    let args = match Invocation::parse()? {
        Invocation::Identity { machine_readable } => {
            print_runtime_identity(machine_readable);
            return Ok(());
        }
        Invocation::Launch(args) => args,
    };
    if args.worker {
        return run_worker(args);
    }
    if args.supervised {
        if args.debugger {
            eprintln!("RUSTY_HOST debugger: worker startup and callback deadlines are disabled; source restaging still replaces the worker");
        }
        return run_supervised_shell(args);
    }
    let diagnostics = ProductDevLog::new(Default::default()).map_err(|error| error.to_string())?;
    let content =
        CsharpProductContent::admit(args.content_root()).map_err(|error| error.to_string())?;
    let (library, runtimeconfig) = args.selected_artifacts()?;
    let mut runtime = match args.loader {
        ProductLoader::NativeAot => CsharpProductRuntime::load_admitted(
            library,
            content,
            args.runtime_config().with_diagnostics(diagnostics.clone()),
        ),
        ProductLoader::CoreClr => CsharpProductRuntime::load_coreclr_admitted(
            library,
            runtimeconfig.expect("CoreCLR Product manifest declares runtimeconfig"),
            content,
            args.runtime_config().with_diagnostics(diagnostics.clone()),
        ),
    }
    .map_err(|error| error.to_string())?;
    let bundle = match &args.product {
        Some(product) => load_bundle(
            &runtime_browser_root()?,
            product,
            runtime.render_resources(),
        )?,
        None => load_legacy_bundle(
            args.bundle_dir.as_deref().expect("legacy bundle path"),
            runtime.render_resources(),
        )?,
    };
    if args.exercise {
        runtime
            .exercise_updates()
            .map_err(|error| error.to_string())?;
    }
    let crossover_durations = args
        .performance_probe
        .map(|iterations| {
            runtime
                .performance_probe_demand(iterations)
                .map(|durations| (iterations, durations))
                .map_err(|error| error.to_string())
        })
        .transpose()?;
    let host = ProductDevHost::start(
        runtime,
        ProductDevHostConfig::new(args.port(), bundle.clone())
            .with_bind_host(args.bind_host())
            .with_live_debug(args.live_debug())
            .with_diagnostics(diagnostics),
    )
    .map_err(|error| error.to_string())?;
    if args.exercise {
        let mut stream = TcpStream::connect(host.address()).map_err(|error| error.to_string())?;
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            host.address()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;
        if !response.starts_with("HTTP/1.1 200") {
            return Err("loopback product host did not serve index.html".to_owned());
        }
        println!(
            "{} lifecycle and loopback host exercise passed at {}",
            args.loader.label(),
            host.origin()
        );
        host.shutdown().map_err(|error| error.to_string())?;
    } else if let Some((iterations, durations)) = crossover_durations {
        println!(
            "RUSTY_PERF {}",
            performance_summary("csharp-rust-crossover", iterations, &durations)
        );
        let output_stream = open_fresh_output_stream(host.address())?;
        let mut host_durations = Vec::with_capacity(iterations as usize);
        for _ in 0..iterations {
            let started = Instant::now();
            post_empty_json(host.address(), "/__rusty/product/runtime/admit-demand-step")?;
            host_durations.push(started.elapsed().as_nanos());
        }
        println!(
            "RUSTY_PERF {}",
            performance_summary("product-dev-host-http", iterations, &host_durations)
        );
        drop(output_stream);
        host.shutdown().map_err(|error| error.to_string())?;
    } else {
        println!(
            "C# {} product host listening at {}",
            args.loader.label(),
            host.origin()
        );
        println!("Press Ctrl+C to stop.");
        wait_for_process_termination(args.supervised, &host);
    }
    Ok(())
}

const WORKER_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_WORKER_INPUT_BATCHES: usize = 256;

#[derive(Default)]
struct WorkerInputMailbox {
    batches: Mutex<VecDeque<(Instant, ProductDevInputBatch)>>,
    last_drained_oldest: Mutex<Option<Instant>>,
    overflowed: AtomicBool,
}

impl WorkerInputMailbox {
    fn enqueue(&self, batch: ProductDevInputBatch) -> bool {
        let Ok(mut batches) = self.batches.lock() else {
            return false;
        };
        if batches.len() == MAX_WORKER_INPUT_BATCHES {
            batches.clear();
            self.overflowed.store(true, Ordering::Release);
            return false;
        }
        batches.push_back((Instant::now(), batch));
        true
    }

    fn drain(&self) -> (Vec<ProductDevInputBatch>, bool) {
        let mut batches = self.batches.lock().expect("worker mailbox lock");
        let overflowed = self.overflowed.swap(false, Ordering::AcqRel);
        *self
            .last_drained_oldest
            .lock()
            .expect("worker mailbox age lock") = batches.front().map(|(at, _)| *at);
        (
            batches.drain(..).map(|(_, batch)| batch).collect(),
            overflowed,
        )
    }

    fn clear(&self) {
        if let Ok(mut batches) = self.batches.lock() {
            batches.clear();
        }
        self.overflowed.store(false, Ordering::Release);
        if let Ok(mut oldest) = self.last_drained_oldest.lock() {
            *oldest = None;
        }
    }
}

/// Foreground `rusty dev` shell.  It owns the one browser listener and its
/// retained projection; the C# runtime itself lives only in the worker below.
fn run_supervised_shell(args: Arguments) -> Result<(), String> {
    let diagnostics = ProductDevLog::new(Default::default()).map_err(|error| error.to_string())?;
    let (runtime, bundle, initial_outputs, worker_outputs, worker_diagnostics, worker_failures) =
        WorkerRuntime::start(&args)?;
    let host = ProductDevHost::start(
        runtime.clone(),
        ProductDevHostConfig::new(args.port(), bundle.clone())
            .with_bind_host(args.bind_host())
            .with_live_debug(args.live_debug())
            .with_diagnostics(diagnostics.clone())
            .with_worker_outputs(
                worker_outputs,
                Arc::clone(&runtime.output_generation),
                runtime.failures.clone(),
                initial_outputs,
                1,
            )
            .with_worker_diagnostics(worker_diagnostics)
            .with_worker_scheduler(Arc::clone(&runtime.scheduler_inflight)),
    )
    .map_err(|error| error.to_string())?;
    runtime
        .activate_current()
        .map_err(|error| format!("{}: {}", error.code(), error.diagnostic()))?;
    println!(
        "C# {} product host listening at {}",
        args.loader.label(),
        host.origin()
    );
    println!("Press Ctrl+C to stop.");
    supervise_worker_replacements(&args, &runtime, &host, &diagnostics, worker_failures)?;
    Ok(())
}

/// The foreground shell accepts exactly one supervisor command family over
/// stdin.  `rusty dev` writes it after staging a new Product directory; EOF
/// remains a clean shell stop.  This is deliberately separate from the
/// worker's typed operation channel and cannot be reached by browser input.
#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum SupervisorCommand {
    ReplaceRuntime { product_directory: PathBuf },
}

fn supervise_worker_replacements(
    shell_args: &Arguments,
    runtime: &WorkerRuntime,
    host: &RunningProductDevHost,
    diagnostics: &ProductDevLog,
    failures: mpsc::Receiver<u64>,
) -> Result<(), String> {
    let termination = install_termination_signal_hook();
    let mut current_product_directory = shell_args.product_path.clone().ok_or(
        "DEV_HOST_SUPERVISOR_REPLACE: supervised shell requires a staged Product directory",
    )?;
    let mut next_runtime_instance_id = shell_args
        .runtime_instance_id
        .map(RuntimeInstanceId::value)
        .unwrap_or(1)
        .saturating_add(1)
        .max(1);
    // One automatic recovery attempt is intentionally small. A repeated
    // worker failure pauses at the stable shell until source restaging gives
    // it a new product to load; no request is replayed in either case.
    let mut automatic_restart_used = false;
    let mut paused_generation = None;
    let (commands_tx, commands_rx) = mpsc::sync_channel(4);
    thread::Builder::new()
        .name("rusty-product-supervisor-control".to_owned())
        .spawn(move || {
            let stdin = std::io::stdin();
            let mut input = stdin.lock();
            loop {
                match read_worker_frame::<SupervisorCommand>(&mut input) {
                    Ok(command) => {
                        if commands_tx.send(Ok(command)).is_err() {
                            return;
                        }
                    }
                    Err(error) if error.code() == "DEV_HOST_WORKER_EOF" => {
                        let _ = commands_tx.send(Err(
                            "DEV_HOST_SUPERVISOR_EOF: supervisor stdin closed".to_owned(),
                        ));
                        return;
                    }
                    Err(error) => {
                        let _ =
                            commands_tx.send(Err(format!("{}: {}", error.code(), error.detail())));
                        return;
                    }
                }
            }
        })
        .map_err(|error| format!("DEV_HOST_SUPERVISOR_CONTROL: {error}"))?;
    loop {
        if termination.load(Ordering::Relaxed) || host.termination_requested() {
            break;
        }
        while let Ok(generation) = failures.try_recv() {
            if generation != runtime.active_generation() {
                continue;
            }
            if automatic_restart_used {
                if let Err(error) = runtime.stop_generation(generation) {
                    publish_shell_diagnostic(diagnostics, "DEV_HOST_WORKER_STOP", &error);
                }
                if paused_generation != Some(generation) {
                    paused_generation = Some(generation);
                    publish_shell_diagnostic(
                        diagnostics,
                        "DEV_HOST_WORKER_PAUSED",
                        &format!(
                            "worker generation {generation} exhausted its one automatic recovery; waiting for source restage"
                        ),
                    );
                }
                continue;
            }
            automatic_restart_used = true;
            let replacement = replace_worker_projection(
                shell_args,
                runtime,
                host,
                current_product_directory.clone(),
                &mut next_runtime_instance_id,
            );
            if let Err(error) = replacement {
                publish_shell_diagnostic(diagnostics, "DEV_HOST_WORKER_RECOVERY", &error);
                paused_generation = Some(runtime.active_generation());
                publish_shell_diagnostic(
                    diagnostics,
                    "DEV_HOST_WORKER_PAUSED",
                    "worker recovery failed; waiting for source restage",
                );
            }
        }
        // Drain exit/protocol failures first so a full bounded channel cannot
        // consume the one scheduler-hang observation when it is taken.
        if let Some(generation) = runtime.expired_scheduler_generation() {
            publish_shell_diagnostic(
                diagnostics,
                "DEV_HOST_WORKER_SCHEDULER_TIMEOUT",
                &format!(
                    "worker generation {generation} did not finish its realtime callback before the operation deadline"
                ),
            );
            let _ = runtime.failures.try_send(generation);
            continue;
        }
        match commands_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(command)) => match command {
                SupervisorCommand::ReplaceRuntime { product_directory } => {
                    current_product_directory = product_directory;
                    automatic_restart_used = false;
                    paused_generation = None;
                    let replacement = replace_worker_projection(
                        shell_args,
                        runtime,
                        host,
                        current_product_directory.clone(),
                        &mut next_runtime_instance_id,
                    );
                    if let Err(error) = replacement {
                        automatic_restart_used = true;
                        publish_shell_diagnostic(
                            diagnostics,
                            "DEV_HOST_SUPERVISOR_REPLACE",
                            &error,
                        );
                        paused_generation = Some(runtime.active_generation());
                        publish_shell_diagnostic(
                            diagnostics,
                            "DEV_HOST_WORKER_PAUSED",
                            "staged worker could not load; waiting for a later source restage",
                        );
                    }
                }
            },
            Ok(Err(error)) if error == "DEV_HOST_SUPERVISOR_EOF: supervisor stdin closed" => break,
            Ok(Err(error)) => {
                publish_shell_diagnostic(diagnostics, "DEV_HOST_SUPERVISOR_CONTROL", &error);
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let reason = if host.termination_requested() {
        "replace-incarnation"
    } else if termination.load(Ordering::Relaxed) {
        "termination-signal"
    } else {
        "supervisor-stdin-closed"
    };
    println!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}");
    Ok(())
}

fn replace_worker_projection(
    shell_args: &Arguments,
    runtime: &WorkerRuntime,
    host: &RunningProductDevHost,
    product_directory: PathBuf,
    next_runtime_instance_id: &mut u64,
) -> Result<(), String> {
    let runtime_instance_id = *next_runtime_instance_id;
    *next_runtime_instance_id = next_runtime_instance_id.saturating_add(1).max(1);
    let replacement = replacement_arguments(shell_args, product_directory, runtime_instance_id)?;
    let prepared = runtime_session::PreparedRuntimeReplacement::prepare(|| {
        let (pending, bundle, baseline) = runtime.prepare_replace(&replacement)?;
        let generation = pending.generation;
        Ok::<_, String>((pending, bundle, baseline, generation))
    })?;
    host.replace_worker_projection(prepared, |pending| runtime.activate_pending(pending))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn publish_shell_diagnostic(diagnostics: &ProductDevLog, code: &str, message: &str) {
    let event = product_dev_host::ProductDevLogEvent::new(
        product_dev_host::ProductDevLogSeverity::Error,
        product_dev_host::ProductDevLogDisposition::Degraded,
        "supervisor",
        code,
        message,
    );
    if let Ok(event) = event {
        let _ = diagnostics.publish(event);
    }
}

fn replacement_arguments(
    shell_args: &Arguments,
    product_directory: PathBuf,
    runtime_instance_id: u64,
) -> Result<Arguments, String> {
    if !product_directory.is_absolute() {
        return Err("DEV_HOST_SUPERVISOR_REPLACE: productDirectory must be absolute".to_owned());
    }
    if runtime_instance_id == 0 {
        return Err("DEV_HOST_SUPERVISOR_REPLACE: runtimeInstanceId must be nonzero".to_owned());
    }
    if !matches!(shell_args.loader, ProductLoader::CoreClr) {
        return Err(
            "DEV_HOST_SUPERVISOR_REPLACE: supervised replacement requires CoreCLR".to_owned(),
        );
    }
    let product = ProductBundle::read(&product_directory)?;
    Ok(Arguments {
        loader: ProductLoader::CoreClr,
        product: Some(product),
        product_path: Some(product_directory),
        library: None,
        runtime_config_path: None,
        bundle_dir: None,
        content_dir: None,
        // Listener configuration was admitted once when the stable shell
        // started; replacement Product metadata never moves that listener.
        port: shell_args.port,
        bind_host: shell_args.bind_host,
        mode: None,
        direct_intents: Vec::new(),
        physical_mappings: Vec::new(),
        legacy_live_debug: false,
        persistence_root: shell_args.persistence_root.clone(),
        content_store_root: shell_args.content_store_root.clone(),
        exercise: false,
        performance_probe: None,
        supervised: false,
        debugger: shell_args.debugger,
        runtime_instance_id: Some(RuntimeInstanceId::new(runtime_instance_id)),
        worker: false,
        worker_channel: None,
    })
}

#[derive(Clone)]
struct WorkerRuntime {
    connection: Arc<Mutex<WorkerConnection>>,
    outputs: mpsc::SyncSender<ProductDevWorkerPublication>,
    output_generation: Arc<AtomicUsize>,
    diagnostics: mpsc::SyncSender<ProductDevWorkerDiagnostic>,
    failures: mpsc::SyncSender<u64>,
    scheduler_inflight: runtime_diagnostics::RuntimeOperationActivity,
    operation_timeout: Option<Duration>,
}

type WorkerStart = (
    WorkerRuntime,
    ProductDevBundle,
    Vec<ProductDevRuntimeOutput>,
    mpsc::Receiver<ProductDevWorkerPublication>,
    mpsc::Receiver<ProductDevWorkerDiagnostic>,
    mpsc::Receiver<u64>,
);

struct PendingWorker {
    connection: WorkerConnection,
    generation: u64,
}

struct WorkerResponse {
    response: ProductDevWorkerResponse,
    connection_output_cursor: Option<u64>,
}

// Scoped to one worker incarnation. Preserve the initiating failure when its
// subsequent socket closure/write errors reach other host operations.
#[derive(Clone, Default)]
struct WorkerTerminalCause(Arc<Mutex<Option<ProductDevRuntimeError>>>);

impl WorkerTerminalCause {
    fn read(&self) -> Option<ProductDevRuntimeError> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn retain(&self, error: ProductDevRuntimeError) -> ProductDevRuntimeError {
        let mut first = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if error.recovery().next_action()
            == product_dev_host::ProductDevNextAction::ReplaceIncarnation
        {
            first.get_or_insert_with(|| error.clone());
        }
        first.clone().unwrap_or(error)
    }
}

struct WorkerConnection {
    child: Child,
    writer: TcpStream,
    responses: mpsc::Receiver<Result<WorkerResponse, ProductDevRuntimeError>>,
    next_request_id: u64,
    operation_timeout: Option<Duration>,
    pending_attribution: Option<product_dev_host::ProductDevUpdateAttribution>,
    terminal_cause: WorkerTerminalCause,
    retiring: Arc<AtomicBool>,
    reader: Option<thread::JoinHandle<()>>,
    generation: u64,
}

impl Drop for WorkerConnection {
    fn drop(&mut self) {
        stop_worker(self);
    }
}

impl WorkerRuntime {
    fn active_generation(&self) -> u64 {
        self.connection
            .lock()
            .map(|connection| connection.generation)
            .unwrap_or_default()
    }

    fn expired_scheduler_generation(&self) -> Option<u64> {
        let timeout = self.operation_timeout?;
        let mut inflight = self.scheduler_inflight.lock().ok()?;
        let (generation, started) = *inflight.as_ref()?;
        if started.elapsed() <= timeout {
            return None;
        }
        *inflight = None;
        Some(generation)
    }

    fn stop_generation(&self, generation: u64) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "DEV_HOST_WORKER_LOCK: worker connection lock is poisoned".to_owned())?;
        if connection.generation != generation {
            return Ok(());
        }
        stop_worker(&mut connection);
        if let Ok(mut inflight) = self.scheduler_inflight.lock() {
            if inflight.is_some_and(|(active_generation, _)| active_generation == generation) {
                *inflight = None;
            }
        }
        Ok(())
    }

    fn start(args: &Arguments) -> Result<WorkerStart, String> {
        let (output_tx, output_rx) = mpsc::sync_channel(16);
        // Output remains fenced until the stable shell atomically publishes
        // the worker's ready bundle and complete baseline.
        let output_generation = Arc::new(AtomicUsize::new(0));
        let (diagnostic_tx, diagnostic_rx) = mpsc::sync_channel(16);
        let (failure_tx, failure_rx) = mpsc::sync_channel(4);
        let scheduler_inflight = Arc::new(Mutex::new(None));
        let (connection, bundle, initial_outputs) = Self::spawn_connection(
            args,
            output_tx.clone(),
            diagnostic_tx.clone(),
            failure_tx.clone(),
            Arc::clone(&scheduler_inflight),
            1,
        )?;
        Ok((
            Self {
                connection: Arc::new(Mutex::new(connection)),
                outputs: output_tx,
                output_generation,
                diagnostics: diagnostic_tx,
                failures: failure_tx,
                scheduler_inflight,
                operation_timeout: args.worker_operation_timeout(),
            },
            bundle,
            initial_outputs,
            output_rx,
            diagnostic_rx,
            failure_rx,
        ))
    }

    /// Stops the retiring worker before loading the replacement and leaves
    /// the shell's listener, history owner, and local output/diagnostic
    /// consumers untouched.  No request is reissued across this boundary.
    fn prepare_replace(
        &self,
        args: &Arguments,
    ) -> Result<
        (
            PendingWorker,
            ProductDevBundle,
            Vec<ProductDevRuntimeOutput>,
        ),
        String,
    > {
        let mut current = self
            .connection
            .lock()
            .map_err(|_| "DEV_HOST_WORKER_LOCK: worker connection lock is poisoned".to_owned())?;
        let retired_generation = current.generation;
        stop_worker(&mut current);
        if let Ok(mut inflight) = self.scheduler_inflight.lock() {
            if inflight.is_some_and(|(generation, _)| generation == retired_generation) {
                *inflight = None;
            }
        }
        let generation = current.generation.saturating_add(1).max(1);
        let (replacement, bundle, initial_outputs) = Self::spawn_connection(
            args,
            self.outputs.clone(),
            self.diagnostics.clone(),
            self.failures.clone(),
            Arc::clone(&self.scheduler_inflight),
            generation,
        )?;
        Ok((
            PendingWorker {
                connection: replacement,
                generation,
            },
            bundle,
            initial_outputs,
        ))
    }

    fn activate_pending(&self, mut pending: PendingWorker) -> Result<(), ProductDevRuntimeError> {
        invoke_connection::<serde_json::Value>(
            &self.failures,
            &mut pending.connection,
            |request_id| ProductDevWorkerRequest::Activate { request_id },
        )?;
        let mut current = self.connection.lock().map_err(|_| {
            worker_runtime_error("DEV_HOST_WORKER_LOCK", "worker connection lock is poisoned")
        })?;
        *current = pending.connection;
        Ok(())
    }

    fn activate_current(&self) -> Result<(), ProductDevRuntimeError> {
        let mut current = self.connection.lock().map_err(|_| {
            worker_runtime_error("DEV_HOST_WORKER_LOCK", "worker connection lock is poisoned")
        })?;
        invoke_connection::<serde_json::Value>(&self.failures, &mut current, |request_id| {
            ProductDevWorkerRequest::Activate { request_id }
        })
        .map(|_| ())
    }

    fn spawn_connection(
        args: &Arguments,
        output_tx: mpsc::SyncSender<ProductDevWorkerPublication>,
        diagnostic_tx: mpsc::SyncSender<ProductDevWorkerDiagnostic>,
        failure_tx: mpsc::SyncSender<u64>,
        scheduler_inflight: runtime_diagnostics::RuntimeOperationActivity,
        generation: u64,
    ) -> Result<
        (
            WorkerConnection,
            ProductDevBundle,
            Vec<ProductDevRuntimeOutput>,
        ),
        String,
    > {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .map_err(|error| format!("DEV_HOST_WORKER_BIND: {error}"))?;
        listener
            .set_nonblocking(false)
            .map_err(|error| format!("DEV_HOST_WORKER_BIND: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("DEV_HOST_WORKER_BIND: {error}"))?;
        let executable = env::current_exe().map_err(|error| {
            format!("DEV_HOST_WORKER_START: cannot resolve host executable: {error}")
        })?;
        let mut child = Command::new(executable)
            .args(worker_arguments(args, address)?)
            // Worker protocol uses its dedicated loopback channel.  Product
            // Console output remains ordinary human stdout/stderr and cannot
            // corrupt a framed response.
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("DEV_HOST_WORKER_START: {error}"))?;
        if let Err(error) = listener.set_nonblocking(true) {
            return worker_start_failed(&mut child, format!("DEV_HOST_WORKER_BIND: {error}"));
        }
        let deadline = args
            .worker_operation_timeout()
            .map(|timeout| Instant::now() + timeout);
        let channel = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("DEV_HOST_WORKER_READY: worker did not connect before the startup deadline".to_owned());
                    }
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            return worker_start_failed(
                                &mut child,
                                format!(
                                    "DEV_HOST_WORKER_READY: worker exited before connecting: {status}"
                                ),
                            );
                        }
                        Ok(None) => {}
                        Err(error) => {
                            return worker_start_failed(
                                &mut child,
                                format!("DEV_HOST_WORKER_READY: {error}"),
                            );
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return worker_start_failed(
                        &mut child,
                        format!("DEV_HOST_WORKER_ACCEPT: {error}"),
                    );
                }
            }
        };
        let mut channel = channel;
        // Framed commands write their length and payload separately. Do not
        // wait for delayed ACKs before delivering these small interactive frames.
        if let Err(error) = channel.set_nodelay(true) {
            return worker_start_failed(&mut child, format!("DEV_HOST_WORKER_CHANNEL: {error}"));
        }
        if let Err(error) = channel.set_read_timeout(args.worker_operation_timeout()) {
            return worker_start_failed(&mut child, format!("DEV_HOST_WORKER_CHANNEL: {error}"));
        }
        let ready = match read_worker_frame::<ProductDevWorkerEvent>(&mut channel) {
            Ok(ready) => ready,
            Err(error) => {
                return worker_start_failed(
                    &mut child,
                    format!("{}: {}", error.code(), error.detail()),
                );
            }
        };
        let ProductDevWorkerEvent::Ready {
            bundle,
            outputs,
            diagnostics,
        } = ready
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("DEV_HOST_WORKER_READY: worker did not send a readiness bundle".to_owned());
        };
        let bundle = match worker_bundle(bundle) {
            Ok(bundle) => bundle,
            Err(error) => return worker_start_failed(&mut child, error),
        };
        let initial_outputs = match worker_outputs(outputs) {
            Ok(outputs) => outputs,
            Err(error) => {
                return worker_start_failed(
                    &mut child,
                    format!("{}: {}", error.code(), error.diagnostic()),
                );
            }
        };
        for diagnostic in diagnostics {
            let _ = diagnostic_tx.try_send(diagnostic);
        }
        if let Err(error) = channel.set_read_timeout(None) {
            return worker_start_failed(&mut child, format!("DEV_HOST_WORKER_CHANNEL: {error}"));
        }
        let writer = match channel.try_clone() {
            Ok(writer) => writer,
            Err(error) => {
                return worker_start_failed(
                    &mut child,
                    format!("DEV_HOST_WORKER_CHANNEL: {error}"),
                );
            }
        };
        if let Err(error) = writer.set_write_timeout(args.worker_operation_timeout()) {
            return worker_start_failed(&mut child, format!("DEV_HOST_WORKER_CHANNEL: {error}"));
        }
        let (response_tx, response_rx) = mpsc::channel();
        let retiring = Arc::new(AtomicBool::new(false));
        let terminal_cause = WorkerTerminalCause::default();
        let lifetime = WorkerReaderLifetime {
            generation,
            retiring: Arc::clone(&retiring),
            terminal_cause: terminal_cause.clone(),
        };
        let reader = match thread::Builder::new()
            .name("rusty-product-worker-reader".to_owned())
            .spawn(move || {
                worker_reader(
                    channel,
                    response_tx,
                    output_tx,
                    diagnostic_tx,
                    failure_tx,
                    scheduler_inflight,
                    lifetime,
                )
            }) {
            Ok(reader) => reader,
            Err(error) => {
                return worker_start_failed(
                    &mut child,
                    format!("DEV_HOST_WORKER_CHANNEL: {error}"),
                );
            }
        };
        Ok((
            WorkerConnection {
                child,
                writer,
                responses: response_rx,
                next_request_id: 1,
                operation_timeout: args.worker_operation_timeout(),
                pending_attribution: None,
                terminal_cause,
                retiring,
                reader: Some(reader),
                generation,
            },
            bundle,
            initial_outputs,
        ))
    }

    fn invoke<T: serde::de::DeserializeOwned>(
        &self,
        request: impl FnOnce(u64) -> ProductDevWorkerRequest,
    ) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError> {
        let mut connection = self.connection.lock().map_err(|_| {
            worker_runtime_error("DEV_HOST_WORKER_LOCK", "worker connection lock is poisoned")
        })?;
        invoke_connection(&self.failures, &mut connection, request)
    }
}

fn invoke_connection<T: serde::de::DeserializeOwned>(
    failures: &mpsc::SyncSender<u64>,
    connection: &mut WorkerConnection,
    request: impl FnOnce(u64) -> ProductDevWorkerRequest,
) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError> {
    if let Some(error) = connection.terminal_cause.read() {
        return Err(error);
    }
    invoke_connection_inner(failures, connection, request)
        .map_err(|error| connection.terminal_cause.retain(error))
}

fn invoke_connection_inner<T: serde::de::DeserializeOwned>(
    failures: &mpsc::SyncSender<u64>,
    connection: &mut WorkerConnection,
    request: impl FnOnce(u64) -> ProductDevWorkerRequest,
) -> Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError> {
    let request_id = connection.next_request_id;
    connection.next_request_id = connection.next_request_id.saturating_add(1).max(1);
    if let Err(error) = write_worker_frame(&mut connection.writer, &request(request_id)) {
        let error = worker_host_error(error);
        let _ = failures.try_send(connection.generation);
        stop_worker(connection);
        return Err(error);
    }
    // A debugger can stop inside any managed callback. Only the explicit
    // development mode waits without a deadline; channel EOF still fails.
    let received = match connection.operation_timeout {
        Some(timeout) => connection.responses.recv_timeout(timeout),
        None => connection
            .responses
            .recv()
            .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
    };
    let response = match received {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => {
            stop_worker(connection);
            let _ = failures.try_send(connection.generation);
            return Err(error);
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            stop_worker(connection);
            let _ = failures.try_send(connection.generation);
            return Err(worker_runtime_error(
                "DEV_HOST_WORKER_TIMEOUT",
                "worker did not complete the active operation before the deadline",
            ));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            stop_worker(connection);
            let _ = failures.try_send(connection.generation);
            return Err(worker_runtime_error(
                "DEV_HOST_WORKER_EOF",
                "worker response reader ended before the active operation completed",
            ));
        }
    };
    let WorkerResponse {
        response,
        connection_output_cursor,
    } = response;
    if response.request_id != request_id {
        stop_worker(connection);
        let _ = failures.try_send(connection.generation);
        return Err(worker_runtime_error(
            "DEV_HOST_WORKER_ORDER",
            "worker response did not match the active operation",
        ));
    }
    connection.pending_attribution = response.attribution;
    if let Some(error) = response.error {
        let error = worker_fault_error(error);
        if error.recovery().next_action()
            == product_dev_host::ProductDevNextAction::ReplaceIncarnation
        {
            stop_worker(connection);
            let _ = failures.try_send(connection.generation);
        }
        return Err(error);
    }
    let result = response.result.ok_or_else(|| {
        worker_runtime_error(
            "DEV_HOST_WORKER_RESPONSE",
            "worker response omitted its concrete result",
        )
    })?;
    let result = serde_json::from_value(result).map_err(|_| {
        worker_runtime_error(
            "DEV_HOST_WORKER_RESPONSE",
            "worker response did not match the requested result family",
        )
    })?;
    let outputs = match worker_publications(response.outputs) {
        Ok(outputs) => outputs,
        Err(error) => {
            stop_worker(connection);
            let _ = failures.try_send(connection.generation);
            return Err(error);
        }
    };
    let receipt = ProductDevRuntimeReceipt::new(result, outputs).map_err(worker_host_error)?;
    Ok(match connection_output_cursor {
        Some(cursor) => receipt.with_connection_output_cursor(cursor),
        None => receipt,
    })
}

struct WorkerReaderLifetime {
    generation: u64,
    retiring: Arc<AtomicBool>,
    terminal_cause: WorkerTerminalCause,
}

fn worker_reader(
    mut channel: TcpStream,
    responses: mpsc::Sender<Result<WorkerResponse, ProductDevRuntimeError>>,
    outputs: mpsc::SyncSender<ProductDevWorkerPublication>,
    diagnostics: mpsc::SyncSender<ProductDevWorkerDiagnostic>,
    failures: mpsc::SyncSender<u64>,
    scheduler_inflight: runtime_diagnostics::RuntimeOperationActivity,
    lifetime: WorkerReaderLifetime,
) {
    let generation = lifetime.generation;
    let mut delivery_started = None;
    loop {
        match read_worker_frame::<ProductDevWorkerEvent>(&mut channel) {
            Ok(ProductDevWorkerEvent::ConnectionResponse(response)) => {
                // The child writes this between serialized publications. Route
                // a marker through the same shell queue as those publications;
                // later ticks must not move this snapshot's subscription cursor.
                let (acknowledged, acknowledgement) = mpsc::sync_channel(1);
                let queued = outputs.try_send(ProductDevWorkerPublication::ConnectionBoundary {
                    generation,
                    acknowledged,
                });
                let cursor = queued.ok().and_then(|()| {
                    acknowledgement
                        .recv_timeout(WORKER_OPERATION_TIMEOUT)
                        .ok()
                        .flatten()
                });
                let Some(cursor) = cursor else {
                    let error = worker_runtime_error(
                        "DEV_HOST_WORKER_CONNECTION_BOUNDARY",
                        "worker connection publication boundary was not acknowledged",
                    );
                    let _ = responses.send(Err(lifetime.terminal_cause.retain(error)));
                    let _ = failures.try_send(generation);
                    return;
                };
                if responses
                    .send(Ok(WorkerResponse {
                        response,
                        connection_output_cursor: Some(cursor),
                    }))
                    .is_err()
                {
                    return;
                }
            }
            Ok(ProductDevWorkerEvent::Response(response)) => {
                if responses
                    .send(Ok(WorkerResponse {
                        response,
                        connection_output_cursor: None,
                    }))
                    .is_err()
                {
                    return;
                }
            }
            Ok(ProductDevWorkerEvent::Outputs { outputs: values }) => {
                let received_at = Instant::now();
                let decoded = worker_outputs(values);
                let decode_duration_us = elapsed_us(received_at);
                match decoded {
                    Ok(decoded) => match outputs.try_send(ProductDevWorkerPublication::Outputs(
                        ProductDevWorkerOutputBatch {
                            generation,
                            outputs: decoded,
                            received_at,
                            decode_duration_us,
                        },
                    )) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Disconnected(_)) => return,
                        Err(mpsc::TrySendError::Full(_)) => {
                            let error = worker_runtime_error(
                                "DEV_HOST_WORKER_OUTPUT_BACKPRESSURE",
                                "shell output receiver did not drain the bounded worker output queue",
                            );
                            let _ = diagnostics.try_send(
                                ProductDevWorkerDiagnostic::from_runtime_error(error.clone()),
                            );
                            let _ = responses.send(Err(lifetime.terminal_cause.retain(error)));
                            let _ = failures.try_send(generation);
                            return;
                        }
                    },
                    Err(error) => {
                        let _ = diagnostics.try_send(
                            ProductDevWorkerDiagnostic::from_runtime_error(error.clone()),
                        );
                        let _ = responses.send(Err(lifetime.terminal_cause.retain(error)));
                        let _ = failures.try_send(generation);
                        return;
                    }
                }
            }
            Ok(ProductDevWorkerEvent::Diagnostics {
                diagnostics: values,
            }) => {
                for diagnostic in values {
                    let _ = diagnostics.try_send(diagnostic);
                }
            }
            Ok(ProductDevWorkerEvent::Health {
                code,
                detail,
                recovery,
            }) => {
                let error = ProductDevRuntimeError::with_recovery(code, detail, recovery)
                    .unwrap_or_else(|_| {
                        worker_runtime_error(
                            "DEV_HOST_WORKER_HEALTH",
                            "worker emitted an invalid bounded health fact",
                        )
                    });
                let _ = diagnostics.try_send(ProductDevWorkerDiagnostic::from_runtime_error(
                    error.clone(),
                ));
                let _ = responses.send(Err(lifetime.terminal_cause.retain(error)));
                let _ = failures.try_send(generation);
            }
            Ok(ProductDevWorkerEvent::UpdateTelemetry { telemetry }) => {
                let delivery_interval_us = delivery_started.take().map(elapsed_us);
                // Same bounded ordered publication queue as outputs. Losing a
                // telemetry observation must not silently make stale timing look current.
                if outputs
                    .try_send(ProductDevWorkerPublication::UpdateTelemetry {
                        generation,
                        telemetry,
                        delivery_interval_us,
                    })
                    .is_err()
                {
                    let error = worker_runtime_error("DEV_HOST_WORKER_TELEMETRY_DROPPED", "shell publication queue could not retain the worker timing observation; displayed sample age may grow");
                    let mut diagnostic = ProductDevWorkerDiagnostic::from_runtime_error(error);
                    diagnostic.severity = product_dev_host::ProductDevLogSeverity::Warning;
                    diagnostic.disposition = product_dev_host::ProductDevLogDisposition::Degraded;
                    let _ = diagnostics.try_send(diagnostic);
                }
            }
            Ok(ProductDevWorkerEvent::SchedulerActivity { active }) => {
                if active {
                    delivery_started = Some(Instant::now());
                }
                if let Ok(mut inflight) = scheduler_inflight.lock() {
                    if active {
                        *inflight = Some((generation, Instant::now()));
                    } else if inflight
                        .is_some_and(|(active_generation, _)| active_generation == generation)
                    {
                        *inflight = None;
                    }
                }
            }
            Ok(ProductDevWorkerEvent::Ready { .. }) => {}
            Err(error) => {
                if lifetime.retiring.load(Ordering::Acquire) {
                    return;
                }
                let error = worker_host_error(error);
                let _ = diagnostics.try_send(ProductDevWorkerDiagnostic::from_runtime_error(
                    error.clone(),
                ));
                let _ = responses.send(Err(lifetime.terminal_cause.retain(error)));
                let _ = failures.try_send(generation);
                return;
            }
        }
    }
}

fn reap_worker(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn worker_start_failed<T>(child: &mut Child, error: String) -> Result<T, String> {
    reap_worker(child);
    Err(error)
}

fn stop_worker(connection: &mut WorkerConnection) {
    connection.retiring.store(true, Ordering::Release);
    reap_worker(&mut connection.child);
    if let Some(reader) = connection.reader.take() {
        let _ = reader.join();
    }
}

fn worker_bundle(bundle: ProductDevWorkerBundle) -> Result<ProductDevBundle, String> {
    let entries = bundle
        .entries
        .into_iter()
        .map(|entry| ProductDevBundleEntry::new(entry.path, entry.content_type, entry.bytes))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    ProductDevBundle::new(entries).map_err(|error| error.to_string())
}

fn worker_output(
    value: serde_json::Value,
) -> Result<ProductDevRuntimeOutput, ProductDevRuntimeError> {
    let bytes = serde_json::to_vec(&value).map_err(|_| {
        worker_runtime_error(
            "DEV_HOST_WORKER_OUTPUT_DECODE",
            "worker output could not be encoded for bounded decoding",
        )
    })?;
    ProductDevRuntimeOutput::decode_json(&bytes).map_err(worker_host_error)
}

fn worker_publications(
    values: Vec<serde_json::Value>,
) -> Result<Vec<runtime_publication::RuntimePublication>, ProductDevRuntimeError> {
    worker_outputs(values)?
        .into_iter()
        .map(|output| output.into_publication().map_err(worker_host_error))
        .collect()
}

/// Decode the full worker output group before admitting it. A complete
/// binding-to-completion baseline may use the dedicated 64 MiB recovery
/// budget; every other worker publication remains in the normal 16 MiB lane.
fn worker_outputs(
    values: Vec<serde_json::Value>,
) -> Result<Vec<ProductDevRuntimeOutput>, ProductDevRuntimeError> {
    let outputs = values
        .into_iter()
        .map(worker_output)
        .collect::<Result<Vec<_>, _>>()?;
    ProductDevRuntimeOutput::validate_output_group(&outputs).map_err(worker_host_error)?;
    Ok(outputs)
}

fn worker_host_error(error: product_dev_host::ProductDevHostError) -> ProductDevRuntimeError {
    worker_runtime_error(error.code(), error.detail())
}

fn worker_runtime_error(
    code: impl Into<String>,
    detail: impl Into<String>,
) -> ProductDevRuntimeError {
    ProductDevRuntimeError::new(code, detail).expect("fixed worker diagnostic is bounded")
}

fn worker_fault_error(fault: ProductDevWorkerFault) -> ProductDevRuntimeError {
    ProductDevRuntimeError::with_recovery(fault.code, fault.diagnostic, fault.recovery)
        .expect("worker fault was admitted before crossing the local channel")
}

impl ProductDevRuntime for WorkerRuntime {
    fn take_update_attribution(&mut self) -> Option<product_dev_host::ProductDevUpdateAttribution> {
        self.connection.lock().ok()?.pending_attribution.take()
    }

    fn realtime_schedule_state(&self) -> ProductDevRuntimeScheduleState {
        // The child owns realtime scheduling.  The shell must never create a
        // second tick loop merely because a product is realtime-configured.
        ProductDevRuntimeScheduleState::Unsupported
    }

    fn connect(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Lifecycle {
            request_id,
            operation: ProductDevWorkerLifecycleOperation::Connect,
            binding: None,
        })
    }

    fn lifecycle(
        &mut self,
        operation: ProductDevLifecycleOperation,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.lifecycle_with_binding(operation, None)
    }

    fn lifecycle_with_binding(
        &mut self,
        operation: ProductDevLifecycleOperation,
        binding: Option<ProductDevRuntimeBinding>,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Lifecycle {
            request_id,
            operation: worker_lifecycle_operation(operation),
            binding,
        })
    }

    fn control(
        &mut self,
        operation: ProductDevControlOperation,
        binding: ProductDevRuntimeBinding,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Control {
            request_id,
            operation: match operation {
                ProductDevControlOperation::Replace => ProductDevWorkerControlOperation::Replace,
                ProductDevControlOperation::Release => ProductDevWorkerControlOperation::Release,
            },
            binding,
        })
    }

    fn recover_input_overflow(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::RecoverInput { request_id })
    }

    fn input(
        &mut self,
        batch: ProductDevInputBatch,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevInputResult>, ProductDevRuntimeError> {
        let payload = serde_json::from_slice(batch.encoded_json().ok_or_else(|| worker_runtime_error(
            "DEV_HOST_WORKER_INPUT",
            "typed input without an admitted host wire form cannot cross the disposable worker boundary",
        ))?).map_err(|_| worker_runtime_error("DEV_HOST_WORKER_INPUT", "input batch could not be decoded"))?;
        self.invoke(|request_id| ProductDevWorkerRequest::Input {
            request_id,
            payload,
        })
    }

    fn execute_debug(
        &mut self,
        command: &str,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Debug {
            request_id,
            command: Some(command.to_owned()),
        })
    }

    fn describe_debug(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugCatalog>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Debug {
            request_id,
            command: None,
        })
    }

    fn report_audio_feedback(
        &mut self,
        feedback: ProductDevAudioFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevAudioFeedbackResult>, ProductDevRuntimeError>
    {
        self.feedback(ProductDevWorkerFeedbackOperation::Audio, feedback)
    }

    fn report_animation_feedback(
        &mut self,
        feedback: ProductDevAnimationFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevAnimationFeedbackResult>, ProductDevRuntimeError>
    {
        self.feedback(ProductDevWorkerFeedbackOperation::Animation, feedback)
    }

    fn report_ghost_plate_feedback(
        &mut self,
        feedback: ProductDevGhostPlateFeedback,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevGhostPlateFeedbackResult>, ProductDevRuntimeError>
    {
        self.feedback(ProductDevWorkerFeedbackOperation::GhostPlate, feedback)
    }

    fn report_renderer_diagnostics(
        &mut self,
        feedback: ProductDevRendererDiagnosticsFeedback,
    ) -> Result<
        ProductDevRuntimeReceipt<ProductDevRendererDiagnosticsFeedbackResult>,
        ProductDevRuntimeError,
    > {
        self.feedback(
            ProductDevWorkerFeedbackOperation::RendererDiagnostics,
            feedback,
        )
    }

    fn advance_realtime(
        &mut self,
        observed_time_ns: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Update {
            request_id,
            operation: ProductDevWorkerUpdateOperation::AdvanceRealtime,
            payload: serde_json::to_value(observed_time_ns)
                .expect("canonical realtime observation encodes"),
        })
    }

    fn admit_demand_step(
        &mut self,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Update {
            request_id,
            operation: ProductDevWorkerUpdateOperation::AdmitDemandStep,
            payload: serde_json::Value::Null,
        })
    }

    fn admit_external_step(
        &mut self,
        step: CanonicalU64,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevOperationResult>, ProductDevRuntimeError> {
        self.invoke(|request_id| ProductDevWorkerRequest::Update {
            request_id,
            operation: ProductDevWorkerUpdateOperation::AdmitExternalStep,
            payload: serde_json::to_value(step).expect("canonical external step encodes"),
        })
    }

    fn complete_timeline(
        &mut self,
        completion: ProductDevTimelineCompletion,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevTimelineCompletionResult>, ProductDevRuntimeError>
    {
        let payload = serde_json::from_slice(completion.encoded_json().ok_or_else(|| worker_runtime_error(
            "DEV_HOST_WORKER_TIMELINE",
            "typed timeline completion without an admitted host wire form cannot cross the disposable worker boundary",
        ))?).map_err(|_| worker_runtime_error("DEV_HOST_WORKER_TIMELINE", "timeline completion could not be decoded"))?;
        self.invoke(|request_id| ProductDevWorkerRequest::Update {
            request_id,
            operation: ProductDevWorkerUpdateOperation::CompleteTimeline,
            payload,
        })
    }
}

impl WorkerRuntime {
    fn feedback<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        operation: ProductDevWorkerFeedbackOperation,
        feedback: T,
    ) -> Result<ProductDevRuntimeReceipt<R>, ProductDevRuntimeError> {
        let payload = serde_json::to_value(feedback).map_err(|_| {
            worker_runtime_error(
                "DEV_HOST_WORKER_FEEDBACK",
                "feedback could not be encoded for the local worker",
            )
        })?;
        self.invoke(|request_id| ProductDevWorkerRequest::Feedback {
            request_id,
            operation,
            payload,
        })
    }
}

fn worker_lifecycle_operation(
    operation: ProductDevLifecycleOperation,
) -> ProductDevWorkerLifecycleOperation {
    match operation {
        ProductDevLifecycleOperation::Start => ProductDevWorkerLifecycleOperation::Start,
        ProductDevLifecycleOperation::Pause => ProductDevWorkerLifecycleOperation::Pause,
        ProductDevLifecycleOperation::Resume => ProductDevWorkerLifecycleOperation::Resume,
        ProductDevLifecycleOperation::Restart => ProductDevWorkerLifecycleOperation::Restart,
        ProductDevLifecycleOperation::Shutdown => ProductDevWorkerLifecycleOperation::Shutdown,
        ProductDevLifecycleOperation::ReportFault => {
            ProductDevWorkerLifecycleOperation::ReportFault
        }
    }
}

fn worker_arguments(args: &Arguments, channel: SocketAddr) -> Result<Vec<String>, String> {
    let product = args.product_path.as_ref().ok_or(
        "DEV_HOST_WORKER_START: supervised worker launch requires a staged Product directory",
    )?;
    let loader = match args.loader {
        ProductLoader::NativeAot => "nativeaot",
        ProductLoader::CoreClr => "coreclr",
    };
    let instance = args.runtime_instance_id.ok_or(
        "DEV_HOST_WORKER_START: supervised worker launch requires a runtime incarnation id",
    )?;
    let mut values = vec![
        "--product".to_owned(),
        product.to_string_lossy().into_owned(),
        "--loader".to_owned(),
        loader.to_owned(),
        "--runtime-instance-id".to_owned(),
        instance.value().to_string(),
        "--worker".to_owned(),
        "--worker-channel".to_owned(),
        channel.to_string(),
    ];
    if let Some(root) = &args.persistence_root {
        values.extend([
            "--persistence-root".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
    }
    if let Some(root) = &args.content_store_root {
        values.extend([
            "--content-store-root".to_owned(),
            root.to_string_lossy().into_owned(),
        ]);
    }
    if args.debugger {
        values.push("--debugger".to_owned());
    }
    Ok(values)
}

fn run_worker(args: Arguments) -> Result<(), String> {
    let diagnostics = ProductDevLog::new(Default::default()).map_err(|error| error.to_string())?;
    let content =
        CsharpProductContent::admit(args.content_root()).map_err(|error| error.to_string())?;
    let (library, runtimeconfig) = args.selected_artifacts()?;
    let runtime = match args.loader {
        ProductLoader::NativeAot => CsharpProductRuntime::load_admitted(
            library,
            content,
            args.runtime_config().with_diagnostics(diagnostics.clone()),
        ),
        ProductLoader::CoreClr => CsharpProductRuntime::load_coreclr_admitted(
            library,
            runtimeconfig.expect("CoreCLR Product manifest declares runtimeconfig"),
            content,
            args.runtime_config().with_diagnostics(diagnostics.clone()),
        ),
    }
    .map_err(|error| error.to_string())?;
    let bundle = match &args.product {
        Some(product) => load_bundle(
            &runtime_browser_root()?,
            product,
            runtime.render_resources(),
        )?,
        None => load_legacy_bundle(
            args.bundle_dir.as_deref().expect("legacy bundle path"),
            runtime.render_resources(),
        )?,
    };
    let address = args
        .worker_channel
        .expect("worker mode requires its local channel");
    if !address.ip().is_loopback() {
        return Err("DEV_HOST_WORKER_CHANNEL: worker channel must be loopback".to_owned());
    }
    let mut channel = TcpStream::connect_timeout(&address, WORKER_OPERATION_TIMEOUT)
        .map_err(|error| format!("DEV_HOST_WORKER_CONNECT: {error}"))?;
    channel
        .set_nodelay(true)
        .map_err(|error| format!("DEV_HOST_WORKER_CHANNEL: {error}"))?;
    let owner = Arc::new(ProductDevOperationOwner::new(runtime));
    let mailbox = Arc::new(WorkerInputMailbox::default());
    let diagnostic_cursor = Arc::new(Mutex::new(None));
    let initial = owner.connect();
    let (outputs, fault) = match initial {
        Ok(receipt) => {
            let (_, outputs) = receipt.into_parts();
            (outputs, None)
        }
        Err(error) => (Vec::new(), Some(error)),
    };
    if let Some(error) = fault {
        return Err(format!("{}: {}", error.code(), error.diagnostic()));
    }
    let ready_diagnostics = drain_worker_diagnostics_shared(&diagnostics, &diagnostic_cursor);
    let bundle = ProductDevWorkerBundle {
        entries: bundle
            .entries()
            .map(|entry| ProductDevWorkerBundleEntry {
                path: entry.path().to_owned(),
                content_type: entry.content_type().to_owned(),
                bytes: entry.bytes().to_vec(),
            })
            .collect(),
    };
    let outputs = worker_publication_values(outputs)?;
    write_worker_frame(
        &mut channel,
        &ProductDevWorkerEvent::Ready {
            bundle,
            outputs,
            diagnostics: ready_diagnostics,
        },
    )
    .map_err(|error| error.to_string())?;
    let writer = Arc::new(Mutex::new(
        channel
            .try_clone()
            .map_err(|error| format!("DEV_HOST_WORKER_CHANNEL: {error}"))?,
    ));
    let scheduler_shutdown = Arc::new(AtomicBool::new(false));
    // Mutation, snapshotting, encoding, and wire publication share one order.
    // The runtime owner alone does not cover encoding after a receipt returns.
    let publication_gate = Arc::new(Mutex::new(()));
    let mut scheduler: Option<thread::JoinHandle<()>> = None;
    loop {
        let request = match read_worker_frame::<ProductDevWorkerRequest>(&mut channel) {
            Ok(request) => request,
            Err(error) if error.code() == "DEV_HOST_WORKER_EOF" => break,
            Err(error) => {
                scheduler_shutdown.store(true, Ordering::Release);
                if let Some(scheduler) = scheduler.take() {
                    let _ = scheduler.join();
                }
                return Err(error.to_string());
            }
        };
        let activate = matches!(request, ProductDevWorkerRequest::Activate { .. });
        if scheduler.is_none() && !activate {
            let result = worker_fault_response(
                worker_request_id(&request),
                ProductDevRuntimeError::new_not_applied(
                    "DEV_HOST_WORKER_NOT_ACTIVE",
                    "worker has not completed the shell projection activation",
                )
                .expect("fixed worker inactive diagnostic is bounded"),
            );
            let write = writer
                .lock()
                .map_err(|_| "DEV_HOST_WORKER_CHANNEL: writer lock is poisoned".to_owned())
                .and_then(|mut writer| {
                    write_worker_frame(&mut *writer, &ProductDevWorkerEvent::Response(result))
                        .map_err(|error| error.to_string())
                });
            write?;
            continue;
        }
        let connection = matches!(
            request,
            ProductDevWorkerRequest::Lifecycle {
                operation: ProductDevWorkerLifecycleOperation::Connect,
                ..
            }
        );
        let publication = publication_gate
            .lock()
            .map_err(|_| "DEV_HOST_WORKER_PUBLICATION: publication lock is poisoned".to_owned())?;
        let mut result = worker_request(&owner, &mailbox, request);
        result.attribution = owner
            .take_update_attribution()
            .map_err(|error| format!("{}: {}", error.code(), error.diagnostic()))?;
        let response = if connection {
            ProductDevWorkerEvent::ConnectionResponse(result)
        } else {
            ProductDevWorkerEvent::Response(result)
        };
        let worker_diagnostics = drain_worker_diagnostics_shared(&diagnostics, &diagnostic_cursor);
        let write = writer
            .lock()
            .map_err(|_| "DEV_HOST_WORKER_CHANNEL: writer lock is poisoned".to_owned())
            .and_then(|mut writer| {
                write_worker_frame(&mut *writer, &response)
                    .and_then(|_| {
                        if worker_diagnostics.is_empty() {
                            Ok(())
                        } else {
                            write_worker_frame(
                                &mut *writer,
                                &ProductDevWorkerEvent::Diagnostics {
                                    diagnostics: worker_diagnostics,
                                },
                            )
                        }
                    })
                    .map_err(|error| error.to_string())
            });
        drop(publication);
        if let Err(error) = write {
            scheduler_shutdown.store(true, Ordering::Release);
            if let Some(scheduler) = scheduler.take() {
                let _ = scheduler.join();
            }
            return Err(error);
        }
        if activate && scheduler.is_none() {
            let owner = Arc::clone(&owner);
            let mailbox = Arc::clone(&mailbox);
            let writer = Arc::clone(&writer);
            let shutdown = Arc::clone(&scheduler_shutdown);
            let diagnostics = diagnostics.clone();
            let diagnostic_cursor = Arc::clone(&diagnostic_cursor);
            let publication_gate = Arc::clone(&publication_gate);
            scheduler = Some(
                thread::Builder::new()
                    .name("rusty-product-worker-scheduler".to_owned())
                    .spawn(move || {
                        worker_scheduler(
                            owner,
                            mailbox,
                            writer,
                            shutdown,
                            diagnostics,
                            diagnostic_cursor,
                            publication_gate,
                        )
                    })
                    .map_err(|error| format!("DEV_HOST_WORKER_SCHEDULER: {error}"))?,
            );
        }
    }
    scheduler_shutdown.store(true, Ordering::Release);
    if let Some(scheduler) = scheduler {
        let _ = scheduler.join();
    }
    Ok(())
}

fn elapsed_us(started: Instant) -> u64 {
    started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64
}

fn worker_scheduler(
    owner: Arc<ProductDevOperationOwner<CsharpProductRuntime>>,
    mailbox: Arc<WorkerInputMailbox>,
    writer: Arc<Mutex<TcpStream>>,
    shutdown: Arc<AtomicBool>,
    diagnostics: ProductDevLog,
    diagnostic_cursor: Arc<Mutex<Option<u64>>>,
    publication_gate: Arc<Mutex<()>>,
) {
    let started = Instant::now();
    while !shutdown.load(Ordering::Acquire) {
        let interval = match owner.realtime_schedule_interval() {
            Ok(Some(interval)) => interval,
            _ => {
                thread::sleep(Duration::from_millis(25));
                continue;
            }
        };
        if !matches!(
            owner.realtime_schedule_state(),
            Ok(ProductDevRuntimeScheduleState::Running)
        ) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        let publication = match publication_gate.lock() {
            Ok(publication) => publication,
            Err(_) => {
                shutdown.store(true, Ordering::Release);
                shutdown_worker_channel(&writer);
                return;
            }
        };
        let observed =
            CanonicalU64::new(started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        let mut input_outputs = Vec::new();
        let mut update_outputs = Vec::new();
        let mut readout = None;
        if !publish_scheduler_activity(&writer, true) {
            shutdown_worker_channel(&writer);
            shutdown.store(true, Ordering::Release);
            return;
        }
        let operation_started = Instant::now();
        let mut input_queue_age_us = None;
        let scheduled = advance_realtime_with_input_and_publish(
            &owner,
            || {
                let drained = mailbox.drain();
                input_queue_age_us = mailbox
                    .last_drained_oldest
                    .lock()
                    .ok()
                    .and_then(|oldest| *oldest)
                    .map(elapsed_us);
                drained
            },
            observed,
            |receipt| {
                let (result, receipt_outputs) = receipt.into_parts();
                input_outputs.push((result, receipt_outputs));
            },
            |receipt| {
                let (result, receipt_outputs) = receipt.into_parts();
                readout = result.readout().cloned();
                update_outputs.extend(receipt_outputs);
            },
            || {},
            || {},
        );
        let operation_duration_us = elapsed_us(operation_started);
        if !publish_scheduler_activity(&writer, false) {
            shutdown_worker_channel(&writer);
            shutdown.store(true, Ordering::Release);
            return;
        }
        match scheduled {
            Ok((input_errors, attribution)) => {
                let mut worker_diagnostics =
                    drain_worker_diagnostics_shared(&diagnostics, &diagnostic_cursor);
                worker_diagnostics.extend(
                    input_errors
                        .into_iter()
                        .map(ProductDevWorkerDiagnostic::from_runtime_error),
                );
                let conversion_started = Instant::now();
                let outputs = match (|| -> Result<_, String> {
                    let mut outputs = Vec::new();
                    for (result, publications) in input_outputs {
                        outputs.push(ProductDevRuntimeOutput::runtime_input_result(result));
                        outputs.extend(publication_wire_outputs(publications)?);
                    }
                    outputs.extend(publication_wire_outputs(update_outputs)?);
                    outputs.push(ProductDevRuntimeOutput::runtime_progress());
                    worker_output_values(outputs)
                })() {
                    Ok(outputs) => outputs,
                    Err(detail) => {
                        let error = worker_runtime_error("DEV_HOST_WORKER_OUTPUT_ENCODE", detail);
                        report_scheduler_failure(&writer, worker_diagnostics, error);
                        shutdown.store(true, Ordering::Release);
                        return;
                    }
                };
                let output_conversion_duration_us = elapsed_us(conversion_started);
                let mut writer = match writer.lock() {
                    Ok(writer) => writer,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let encode_write_started = Instant::now();
                if write_worker_frame(&mut *writer, &ProductDevWorkerEvent::Outputs { outputs })
                    .is_err()
                {
                    let _ = writer.shutdown(Shutdown::Both);
                    shutdown.store(true, Ordering::Release);
                    return;
                }
                let telemetry = ProductDevWorkerUpdateTelemetry {
                    worker_pid: CanonicalU64::new(u64::from(std::process::id())),
                    readout,
                    attribution,
                    phases: runtime_diagnostics::RuntimeWorkerPhases {
                        operation_duration_us: CanonicalU64::new(operation_duration_us),
                        output_conversion_duration_us: CanonicalU64::new(
                            output_conversion_duration_us,
                        ),
                        output_encode_write_duration_us: CanonicalU64::new(elapsed_us(
                            encode_write_started,
                        )),
                        input_queue_age_us: input_queue_age_us.map(CanonicalU64::new),
                    },
                };
                if write_worker_frame(
                    &mut *writer,
                    &ProductDevWorkerEvent::UpdateTelemetry {
                        telemetry: Box::new(telemetry),
                    },
                )
                .is_err()
                {
                    let _ = writer.shutdown(Shutdown::Both);
                    shutdown.store(true, Ordering::Release);
                    return;
                }
                if !worker_diagnostics.is_empty()
                    && write_worker_frame(
                        &mut *writer,
                        &ProductDevWorkerEvent::Diagnostics {
                            diagnostics: worker_diagnostics,
                        },
                    )
                    .is_err()
                {
                    let _ = writer.shutdown(Shutdown::Both);
                    shutdown.store(true, Ordering::Release);
                    return;
                }
            }
            Err(error) => {
                let worker_diagnostics =
                    drain_worker_diagnostics_shared(&diagnostics, &diagnostic_cursor);
                report_scheduler_failure(&writer, worker_diagnostics, error);
                shutdown.store(true, Ordering::Release);
                return;
            }
        }
        drop(publication);
        thread::sleep(interval);
    }
}

fn report_scheduler_failure(
    writer: &Arc<Mutex<TcpStream>>,
    mut diagnostics: Vec<ProductDevWorkerDiagnostic>,
    error: ProductDevRuntimeError,
) {
    diagnostics.push(ProductDevWorkerDiagnostic::from_runtime_error(
        error.clone(),
    ));
    if let Ok(mut writer) = writer.lock() {
        let _ = write_worker_frame(
            &mut *writer,
            &ProductDevWorkerEvent::Diagnostics { diagnostics },
        );
        let _ = write_worker_frame(
            &mut *writer,
            &ProductDevWorkerEvent::Health {
                code: error.code().to_owned(),
                detail: error.diagnostic().to_owned(),
                recovery: error.recovery(),
            },
        );
        let _ = writer.shutdown(Shutdown::Both);
    }
}

fn publish_scheduler_activity(writer: &Arc<Mutex<TcpStream>>, active: bool) -> bool {
    writer
        .lock()
        .ok()
        .and_then(|mut writer| {
            write_worker_frame(
                &mut *writer,
                &ProductDevWorkerEvent::SchedulerActivity { active },
            )
            .ok()
        })
        .is_some()
}

fn shutdown_worker_channel(writer: &Arc<Mutex<TcpStream>>) {
    let writer = match writer.lock() {
        Ok(writer) => writer,
        Err(poisoned) => poisoned.into_inner(),
    };
    let _ = writer.shutdown(Shutdown::Both);
}

fn drain_worker_diagnostics(
    diagnostics: &ProductDevLog,
    cursor: &mut Option<u64>,
) -> Vec<ProductDevWorkerDiagnostic> {
    let batch = diagnostics.read_after(*cursor);
    *cursor = Some(batch.next_cursor);
    batch
        .events
        .into_iter()
        .map(ProductDevWorkerDiagnostic::from_log_event)
        .collect()
}

fn drain_worker_diagnostics_shared(
    diagnostics: &ProductDevLog,
    cursor: &Mutex<Option<u64>>,
) -> Vec<ProductDevWorkerDiagnostic> {
    let Ok(mut cursor) = cursor.lock() else {
        return Vec::new();
    };
    drain_worker_diagnostics(diagnostics, &mut cursor)
}

fn publication_wire_outputs(
    publications: Vec<runtime_publication::RuntimePublication>,
) -> Result<Vec<ProductDevRuntimeOutput>, String> {
    publications
        .into_iter()
        .map(|publication| {
            ProductDevRuntimeOutput::from_publication(publication)
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn worker_publication_values(
    publications: Vec<runtime_publication::RuntimePublication>,
) -> Result<Vec<serde_json::Value>, String> {
    worker_output_values(publication_wire_outputs(publications)?)
}

fn worker_output_values(
    outputs: Vec<ProductDevRuntimeOutput>,
) -> Result<Vec<serde_json::Value>, String> {
    ProductDevRuntimeOutput::validate_output_group(&outputs).map_err(|error| error.to_string())?;
    let mut values = Vec::with_capacity(outputs.len());
    for output in outputs {
        let bytes = serde_json::to_vec(&output).map_err(|error| error.to_string())?;
        values.push(serde_json::from_slice(&bytes).map_err(|error| error.to_string())?);
    }
    Ok(values)
}

fn bounded_worker_payload(
    payload: &serde_json::Value,
    code: &str,
) -> Result<Vec<u8>, ProductDevRuntimeError> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|_| worker_runtime_error(code, "worker payload could not be encoded"))?;
    if bytes.len() > product_dev_host::MAX_REQUEST_BODY_BYTES {
        return Err(worker_runtime_error(
            code,
            "worker payload exceeds the normal host request bound",
        ));
    }
    Ok(bytes)
}

fn worker_request(
    owner: &ProductDevOperationOwner<CsharpProductRuntime>,
    mailbox: &WorkerInputMailbox,
    request: ProductDevWorkerRequest,
) -> ProductDevWorkerResponse {
    let request_id = worker_request_id(&request);
    match request {
        ProductDevWorkerRequest::Lifecycle {
            operation, binding, ..
        } => match operation {
            ProductDevWorkerLifecycleOperation::Connect => {
                worker_receipt(request_id, owner.connect())
            }
            _ => {
                mailbox.clear();
                worker_receipt(
                    request_id,
                    owner.lifecycle_with_binding(worker_lifecycle(operation), binding),
                )
            }
        },
        ProductDevWorkerRequest::Control {
            operation, binding, ..
        } => {
            mailbox.clear();
            worker_receipt(
                request_id,
                owner.control(worker_control(operation), binding),
            )
        }
        ProductDevWorkerRequest::RecoverInput { .. } => {
            mailbox.clear();
            worker_receipt(request_id, owner.recover_input_overflow())
        }
        ProductDevWorkerRequest::Input { payload, .. } => {
            let result = bounded_worker_payload(&payload, "DEV_HOST_WORKER_INPUT")
                .and_then(|bytes| {
                    ProductDevInputBatch::decode_json(&bytes).map_err(worker_host_error)
                })
                .and_then(|batch| {
                    if matches!(
                        owner.realtime_schedule_state(),
                        Ok(ProductDevRuntimeScheduleState::Unsupported)
                    ) {
                        return owner.input(batch);
                    }
                    let count = batch.events().len();
                    if mailbox.enqueue(batch) {
                        ProductDevInputResult::queued(count)
                            .map_err(worker_host_error)
                            .and_then(|result| {
                                ProductDevRuntimeReceipt::new(result, Vec::new())
                                    .map_err(worker_host_error)
                            })
                    } else {
                        ProductDevInputResult::mailbox_full(count)
                            .map_err(worker_host_error)
                            .and_then(|result| {
                                ProductDevRuntimeReceipt::new(result, Vec::new())
                                    .map_err(worker_host_error)
                            })
                    }
                });
            worker_receipt(request_id, result)
        }
        ProductDevWorkerRequest::Update {
            operation, payload, ..
        } => match operation {
            ProductDevWorkerUpdateOperation::AdvanceRealtime => {
                let result = bounded_worker_payload(&payload, "DEV_HOST_WORKER_UPDATE")
                    .and_then(|bytes| {
                        CanonicalU64::decode_json(&bytes)
                            .map_err(product_dev_host::ProductDevHostError::from)
                            .map_err(worker_host_error)
                    })
                    .and_then(|time| owner.advance_realtime(time));
                worker_receipt(request_id, result)
            }
            ProductDevWorkerUpdateOperation::AdmitDemandStep => {
                worker_receipt(request_id, owner.admit_demand_step())
            }
            ProductDevWorkerUpdateOperation::AdmitExternalStep => {
                let result = bounded_worker_payload(&payload, "DEV_HOST_WORKER_UPDATE")
                    .and_then(|bytes| {
                        CanonicalU64::decode_json(&bytes)
                            .map_err(product_dev_host::ProductDevHostError::from)
                            .map_err(worker_host_error)
                    })
                    .and_then(|step| owner.admit_external_step(step));
                worker_receipt(request_id, result)
            }
            ProductDevWorkerUpdateOperation::CompleteTimeline => {
                let result = bounded_worker_payload(&payload, "DEV_HOST_WORKER_UPDATE")
                    .and_then(|bytes| {
                        ProductDevTimelineCompletion::decode_json(&bytes).map_err(worker_host_error)
                    })
                    .and_then(|completion| owner.complete_timeline(completion));
                worker_receipt(request_id, result)
            }
        },
        ProductDevWorkerRequest::Debug { command, .. } => match command {
            Some(command) if command.len() <= product_dev_host::MAX_REQUEST_BODY_BYTES => {
                worker_receipt(request_id, owner.execute_debug(&command))
            }
            Some(_) => worker_fault_response(
                request_id,
                worker_runtime_error(
                    "DEV_HOST_WORKER_DEBUG",
                    "debug command exceeds the host request bound",
                ),
            ),
            None => worker_receipt(request_id, owner.describe_debug()),
        },
        ProductDevWorkerRequest::Health { .. } => ProductDevWorkerResponse {
            attribution: None,
            request_id,
            result: Some(serde_json::json!({ "ready": true })),
            outputs: Vec::new(),
            error: None,
        },
        ProductDevWorkerRequest::Activate { .. } => ProductDevWorkerResponse {
            attribution: None,
            request_id,
            result: Some(serde_json::json!({ "active": true })),
            outputs: Vec::new(),
            error: None,
        },
        ProductDevWorkerRequest::Shutdown { .. } => worker_receipt(
            request_id,
            owner.lifecycle(ProductDevLifecycleOperation::Shutdown),
        ),
        ProductDevWorkerRequest::Feedback {
            operation, payload, ..
        } => match bounded_worker_payload(&payload, "DEV_HOST_WORKER_FEEDBACK") {
            Err(error) => worker_fault_response(request_id, error),
            Ok(_) => match operation {
                ProductDevWorkerFeedbackOperation::Audio => worker_feedback(
                    request_id,
                    payload,
                    ProductDevAudioFeedback::validate,
                    |feedback| owner.report_audio_feedback(feedback),
                ),
                ProductDevWorkerFeedbackOperation::Animation => worker_feedback(
                    request_id,
                    payload,
                    ProductDevAnimationFeedback::validate,
                    |feedback| owner.report_animation_feedback(feedback),
                ),
                ProductDevWorkerFeedbackOperation::GhostPlate => worker_feedback(
                    request_id,
                    payload,
                    ProductDevGhostPlateFeedback::validate,
                    |feedback| owner.report_ghost_plate_feedback(feedback),
                ),
                ProductDevWorkerFeedbackOperation::RendererDiagnostics => worker_feedback(
                    request_id,
                    payload,
                    ProductDevRendererDiagnosticsFeedback::validate,
                    |feedback| owner.report_renderer_diagnostics(feedback),
                ),
            },
        },
    }
}

fn worker_request_id(request: &ProductDevWorkerRequest) -> u64 {
    match request {
        ProductDevWorkerRequest::Lifecycle { request_id, .. }
        | ProductDevWorkerRequest::Control { request_id, .. }
        | ProductDevWorkerRequest::RecoverInput { request_id, .. }
        | ProductDevWorkerRequest::Input { request_id, .. }
        | ProductDevWorkerRequest::Update { request_id, .. }
        | ProductDevWorkerRequest::Debug { request_id, .. }
        | ProductDevWorkerRequest::Feedback { request_id, .. }
        | ProductDevWorkerRequest::Health { request_id }
        | ProductDevWorkerRequest::Activate { request_id }
        | ProductDevWorkerRequest::Shutdown { request_id } => *request_id,
    }
}

fn worker_lifecycle(operation: ProductDevWorkerLifecycleOperation) -> ProductDevLifecycleOperation {
    match operation {
        ProductDevWorkerLifecycleOperation::Connect => ProductDevLifecycleOperation::Start,
        ProductDevWorkerLifecycleOperation::Start => ProductDevLifecycleOperation::Start,
        ProductDevWorkerLifecycleOperation::Pause => ProductDevLifecycleOperation::Pause,
        ProductDevWorkerLifecycleOperation::Resume => ProductDevLifecycleOperation::Resume,
        ProductDevWorkerLifecycleOperation::Restart => ProductDevLifecycleOperation::Restart,
        ProductDevWorkerLifecycleOperation::Shutdown => ProductDevLifecycleOperation::Shutdown,
        ProductDevWorkerLifecycleOperation::ReportFault => {
            ProductDevLifecycleOperation::ReportFault
        }
    }
}

fn worker_control(operation: ProductDevWorkerControlOperation) -> ProductDevControlOperation {
    match operation {
        ProductDevWorkerControlOperation::Replace => ProductDevControlOperation::Replace,
        ProductDevWorkerControlOperation::Release => ProductDevControlOperation::Release,
    }
}

fn worker_receipt<T: serde::Serialize>(
    request_id: u64,
    result: Result<ProductDevRuntimeReceipt<T>, ProductDevRuntimeError>,
) -> ProductDevWorkerResponse {
    match result {
        Ok(receipt) => {
            let (result, outputs) = receipt.into_parts();
            let response = serde_json::to_value(result)
                .map_err(|error| error.to_string())
                .and_then(|result| {
                    worker_publication_values(outputs).map(|outputs| (result, outputs))
                });
            match response {
                Ok((result, outputs)) => ProductDevWorkerResponse {
                    attribution: None,
                    request_id,
                    result: Some(result),
                    outputs,
                    error: None,
                },
                Err(error) => worker_fault_response(
                    request_id,
                    worker_runtime_error("DEV_HOST_WORKER_ENCODE", error.to_string()),
                ),
            }
        }
        Err(error) => worker_fault_response(request_id, error),
    }
}

fn worker_fault_response(
    request_id: u64,
    error: ProductDevRuntimeError,
) -> ProductDevWorkerResponse {
    ProductDevWorkerResponse {
        attribution: None,
        request_id,
        result: None,
        outputs: Vec::new(),
        error: Some(ProductDevWorkerFault {
            code: error.code().to_owned(),
            diagnostic: error.diagnostic().to_owned(),
            recovery: error.recovery(),
        }),
    }
}

fn worker_feedback<T, R, V, F>(
    request_id: u64,
    payload: serde_json::Value,
    validate: V,
    apply: F,
) -> ProductDevWorkerResponse
where
    T: serde::de::DeserializeOwned,
    R: serde::Serialize,
    V: FnOnce(&T) -> Result<(), product_dev_host::ProductDevHostError>,
    F: FnOnce(T) -> Result<ProductDevRuntimeReceipt<R>, ProductDevRuntimeError>,
{
    let result = serde_json::from_value(payload)
        .map_err(|_| {
            worker_runtime_error("DEV_HOST_WORKER_FEEDBACK", "feedback payload is invalid")
        })
        .and_then(|feedback: T| {
            validate(&feedback)
                .map_err(worker_host_error)
                .map(|_| feedback)
        })
        .and_then(apply);
    worker_receipt(request_id, result)
}

#[allow(
    clippy::large_enum_variant,
    reason = "the parsed launch arguments remain inline so command dispatch transfers one owned parse result without another allocation"
)]
enum Invocation {
    Identity { machine_readable: bool },
    Launch(Arguments),
}

impl Invocation {
    fn parse() -> Result<Self, String> {
        Self::parse_from(env::args().skip(1))
    }

    fn parse_from(values: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let values = values.into_iter().collect::<Vec<_>>();
        match values.as_slice() {
            [flag] if flag == "--identity" => Ok(Self::Identity {
                machine_readable: true,
            }),
            [flag] if flag == "--version" => Ok(Self::Identity {
                machine_readable: false,
            }),
            _ => Arguments::parse_from(values).map(Self::Launch),
        }
    }
}

fn print_runtime_identity(machine_readable: bool) {
    let identity = product_host_runtime_identity();
    let fingerprint = identity.fingerprint_hex();
    if machine_readable {
        println!(
            "{}",
            serde_json::json!({
                "artifact": "rusty.product.runtime-identity",
                "schemaVersion": 1,
                "host": "rusty-product-host",
                "hostVersion": env!("CARGO_PKG_VERSION"),
                "target": "linux-x64",
                "abi": {
                    "protocolVersion": identity.protocol_version,
                    "engineApiBytes": identity.engine_api_bytes,
                    "productApiBytes": identity.product_api_bytes,
                    "fingerprint": fingerprint,
                    "buildIdentity": identity.build_identity,
                },
            })
        );
    } else {
        println!(
            "rusty-product-host {} (linux-x64; ABI v{}; {})",
            env!("CARGO_PKG_VERSION"),
            identity.protocol_version,
            fingerprint,
        );
    }
}

/// The standard host is owned by its foreground process supervisor. In
/// particular, service launchers commonly provide a closed stdin, so EOF must
/// not be interpreted as a request to shut the host down. The `rusty dev`
/// supervisor opts in explicitly and retains the pipe while the child is live;
/// closing it is its cross-platform clean-replacement signal. Returning here
/// lets `RunningProductDevHost` and then `CsharpProductRuntime` execute their
/// normal shutdown/drop ordering before the process exits.
fn wait_for_process_termination(supervised: bool, host: &RunningProductDevHost) {
    let termination = install_termination_signal_hook();
    if supervised {
        // Keep stdin as the supervisor's clean-stop mechanism, but read it on
        // a helper so a terminal runtime recovery can wake this foreground
        // owner without waiting for the supervisor to close the pipe.
        let supervisor_stdin_closed = Arc::new(AtomicBool::new(false));
        let reader_closed = Arc::clone(&supervisor_stdin_closed);
        std::thread::spawn(move || {
            let mut input = std::io::stdin();
            let mut byte = [0_u8; 1];
            while input.read(&mut byte).is_ok_and(|count| count != 0) {}
            reader_closed.store(true, Ordering::Release);
        });
        while !termination.load(Ordering::Relaxed)
            && !supervisor_stdin_closed.load(Ordering::Acquire)
            && !host.termination_requested()
        {
            std::thread::park_timeout(std::time::Duration::from_millis(50));
        }
        let reason = if host.termination_requested() {
            "replace-incarnation"
        } else if termination.load(Ordering::Relaxed) {
            "termination-signal"
        } else {
            "supervisor-stdin-closed"
        };
        println!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}");
    } else {
        while !termination.load(Ordering::Relaxed) && !host.termination_requested() {
            std::thread::park_timeout(std::time::Duration::from_millis(100));
        }
        let reason = if host.termination_requested() {
            "replace-incarnation"
        } else {
            "termination-signal"
        };
        println!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}");
    }
}

#[cfg(unix)]
fn install_termination_signal_hook() -> Arc<AtomicBool> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};

    let requested = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&requested))
        .expect("fixed SIGINT shutdown hook registration");
    signal_hook::flag::register(SIGTERM, Arc::clone(&requested))
        .expect("fixed SIGTERM shutdown hook registration");
    requested
}

#[cfg(not(unix))]
fn install_termination_signal_hook() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn open_fresh_output_stream(address: SocketAddr) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
    let request = format!(
        "GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: {address}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let mut headers = Vec::with_capacity(512);
    let mut byte = [0_u8; 1];
    while !headers.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .map_err(|error| format!("performance probe output stream failed: {error}"))?;
        headers.push(byte[0]);
        if headers.len() > 16 * 1024 {
            return Err("performance probe output stream headers exceeded 16 KiB".to_owned());
        }
    }
    if !headers.starts_with(b"HTTP/1.1 200") {
        return Err(format!(
            "performance probe output stream failed: {}",
            String::from_utf8_lossy(&headers)
                .lines()
                .next()
                .unwrap_or("empty response")
        ));
    }
    stream
        .set_read_timeout(None)
        .map_err(|error| error.to_string())?;
    Ok(stream)
}

fn post_empty_json(address: SocketAddr, path: &str) -> Result<(), String> {
    let mut stream = TcpStream::connect(address).map_err(|error| error.to_string())?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| error.to_string())?;
    if !response.starts_with("HTTP/1.1 200") {
        return Err(format!(
            "performance probe request {path} failed: {}",
            response.lines().next().unwrap_or("empty response")
        ));
    }
    Ok(())
}

fn performance_summary(lane: &str, iterations: u32, durations: &[u128]) -> serde_json::Value {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    let value_at = |fraction: f64| -> f64 {
        let index = ((sorted.len().saturating_sub(1)) as f64 * fraction).round() as usize;
        sorted[index] as f64 / 1_000_000.0
    };
    let mean = sorted.iter().copied().sum::<u128>() as f64 / sorted.len() as f64 / 1_000_000.0;
    serde_json::json!({
        "schemaVersion": 1,
        "lane": lane,
        "iterations": iterations,
        "unit": "milliseconds",
        "minimum": value_at(0.0),
        "median": value_at(0.5),
        "p95": value_at(0.95),
        "maximum": value_at(1.0),
        "mean": mean,
    })
}

#[derive(Debug)]
struct Arguments {
    loader: ProductLoader,
    product: Option<ProductBundle>,
    product_path: Option<PathBuf>,
    library: Option<PathBuf>,
    runtime_config_path: Option<PathBuf>,
    bundle_dir: Option<PathBuf>,
    content_dir: Option<PathBuf>,
    port: u16,
    bind_host: Ipv4Addr,
    mode: Option<RuntimeMode>,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    physical_mappings: Vec<RuntimeInputMapping>,
    legacy_live_debug: bool,
    persistence_root: Option<PathBuf>,
    content_store_root: Option<PathBuf>,
    exercise: bool,
    performance_probe: Option<u32>,
    supervised: bool,
    debugger: bool,
    runtime_instance_id: Option<RuntimeInstanceId>,
    worker: bool,
    worker_channel: Option<SocketAddr>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StagedLaunchInput {
    artifact: String,
    schema_version: u32,
    loader: String,
}

impl StagedLaunchInput {
    const ARTIFACT: &'static str = "rusty.product.runtime-launch";
    fn read(path: &Path) -> Result<ProductLoader, String> {
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "could not read --staged-launch `{}`: {error}",
                path.display()
            )
        })?;
        let input: Self = serde_json::from_slice(&bytes).map_err(|error| {
            format!("--staged-launch `{}` is not valid: {error}", path.display())
        })?;
        if input.artifact != Self::ARTIFACT || input.schema_version != 1 {
            return Err(format!(
                "--staged-launch `{}` must declare artifact `{}` with schemaVersion 1",
                path.display(),
                Self::ARTIFACT
            ));
        }
        ProductLoader::parse(&input.loader)
            .map_err(|_| "--staged-launch loader must be nativeaot or coreclr".to_owned())
    }
}

#[derive(Clone, Copy, Debug)]
enum ProductLoader {
    NativeAot,
    CoreClr,
}

impl ProductLoader {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "nativeaot" => Ok(Self::NativeAot),
            "coreclr" => Ok(Self::CoreClr),
            _ => Err("--loader must be nativeaot or coreclr".to_owned()),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::NativeAot => "NativeAOT",
            Self::CoreClr => "CoreCLR",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RuntimeMode {
    Realtime,
    Demand,
    External,
}

impl RuntimeMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "realtime" => Ok(Self::Realtime),
            "demand" => Ok(Self::Demand),
            "external" => Ok(Self::External),
            _ => Err("--mode must be realtime, demand, or external".to_owned()),
        }
    }
    fn lifecycle_config(self) -> runtime_lifecycle::RuntimeLifecycleConfig {
        match self {
            Self::Realtime => CsharpProductRuntime::standard_realtime_config(),
            Self::Demand => runtime_lifecycle::RuntimeLifecycleConfig::Demand,
            Self::External => runtime_lifecycle::RuntimeLifecycleConfig::External,
        }
    }
}

impl Arguments {
    fn worker_operation_timeout(&self) -> Option<Duration> {
        (!self.debugger).then_some(WORKER_OPERATION_TIMEOUT)
    }

    fn runtime_config(&self) -> CsharpProductRuntimeConfig {
        let (direct_intents, physical_mappings) = self.input_configuration();
        let lifecycle = match &self.product {
            Some(product) => product.lifecycle,
            None => self
                .mode
                .expect("legacy mode is required")
                .lifecycle_config(),
        };
        let mut config = CsharpProductRuntimeConfig::new(
            self.runtime_instance_id
                .unwrap_or_else(next_direct_runtime_instance_id),
            lifecycle,
            direct_intents,
        )
        .with_physical_mappings(physical_mappings);
        if let Some(root) = &self.persistence_root {
            config = config.with_persistence_root(root.clone());
        }
        if let Some(root) = &self.content_store_root {
            config = config.with_content_store_root(root.clone());
        }
        config
    }

    fn input_configuration(&self) -> (Vec<DirectInputIntentDescriptor>, Vec<RuntimeInputMapping>) {
        let (mut direct_intents, mut physical_mappings) = match &self.product {
            Some(product) => (
                product.direct_intents.clone(),
                product.physical_mappings.clone(),
            ),
            None => (self.direct_intents.clone(), self.physical_mappings.clone()),
        };
        if self.product.is_none() && self.exercise {
            if !direct_intents
                .iter()
                .any(|descriptor| descriptor.id() == "runtime.exercise.move")
            {
                direct_intents.push(
                    DirectInputIntentDescriptor::new(
                        "runtime.exercise.move",
                        IntentValueKind::Digital,
                    )
                    .expect("fixed exercise mapping intent"),
                );
            }
            physical_mappings.push(
                RuntimeInputMapping::new(
                    "runtime.exercise.move",
                    "runtime.exercise.move",
                    RuntimeInputTrigger::Key {
                        code: KeyboardControl::KeyW,
                        edge: InputEdge::Held,
                        chord: Vec::new(),
                        context: None,
                    },
                )
                .expect("fixed exercise physical mapping"),
            );
        }
        (direct_intents, physical_mappings)
    }

    fn selected_artifacts(&self) -> Result<(&Path, Option<&Path>), String> {
        match &self.product {
            Some(product) => product.selected_artifacts(self.loader),
            None => Ok((
                self.library.as_deref().expect("legacy library required"),
                self.runtime_config_path.as_deref(),
            )),
        }
    }

    fn content_root(&self) -> &Path {
        self.product
            .as_ref()
            .map(|product| product.content_root.as_path())
            .unwrap_or_else(|| {
                self.content_dir
                    .as_deref()
                    .expect("legacy content required")
            })
    }
    fn port(&self) -> u16 {
        self.product
            .as_ref()
            .map_or(self.port, |product| product.port)
    }
    fn bind_host(&self) -> Ipv4Addr {
        self.product
            .as_ref()
            .map_or(self.bind_host, |product| product.bind_host)
    }
    fn live_debug(&self) -> bool {
        self.product
            .as_ref()
            .is_some_and(|product| product.live_debug)
            || self.legacy_live_debug
    }

    fn parse_from(values: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut loader = None;
        let mut product = None;
        let mut staged_launch = None;
        let mut library = None;
        let mut runtime_config_path = None;
        let mut bundle_dir = None;
        let mut content_dir = None;
        let mut port = 0;
        let mut bind_host = Ipv4Addr::LOCALHOST;
        let mut mode = None;
        let mut direct_intents = Vec::new();
        let mut physical_mappings = Vec::new();
        let mut persistence_root = None;
        let mut content_store_root = None;
        let mut live_debug = false;
        let mut exercise = false;
        let mut performance_probe = None;
        let mut supervised = false;
        let mut debugger = false;
        let mut runtime_instance_id = None;
        let mut worker = false;
        let mut worker_channel = None;
        let mut values = values.into_iter();
        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--loader" => {
                    loader = Some(ProductLoader::parse(
                        &values.next().ok_or("--loader requires a value")?,
                    )?)
                }
                "--product" => {
                    product = Some(PathBuf::from(
                        values.next().ok_or("--product requires a directory")?,
                    ))
                }
                "--staged-launch" => {
                    staged_launch = Some(PathBuf::from(
                        values.next().ok_or("--staged-launch requires a value")?,
                    ))
                }
                "--library" => library = values.next().map(PathBuf::from),
                "--runtimeconfig" => runtime_config_path = values.next().map(PathBuf::from),
                "--bundle-dir" => bundle_dir = values.next().map(PathBuf::from),
                "--content-dir" => content_dir = values.next().map(PathBuf::from),
                "--port" => {
                    port = values
                        .next()
                        .ok_or("--port requires a value")?
                        .parse()
                        .map_err(|_| "--port must be a u16")?
                }
                "--bind-host" => {
                    bind_host = values
                        .next()
                        .ok_or("--bind-host requires an IPv4 address")?
                        .parse()
                        .map_err(|_| "--bind-host must be an IPv4 address")?
                }
                "--mode" => {
                    mode = Some(RuntimeMode::parse(
                        &values.next().ok_or("--mode requires a value")?,
                    )?)
                }
                "--live-debug" => live_debug = true,
                "--direct-intent" => {
                    direct_intents.push(parse_direct_intent(&values.next().ok_or(
                        "--direct-intent requires id=digital, id=axis, or id=payload:contract",
                    )?)?)
                }
                "--physical-mapping" => {
                    if physical_mappings.len() == MAX_PHYSICAL_MAPPINGS {
                        return Err(format!(
                            "--physical-mapping accepts at most {MAX_PHYSICAL_MAPPINGS} declarations"
                        ));
                    }
                    physical_mappings.push(parse_physical_mapping(
                        &values
                            .next()
                            .ok_or("--physical-mapping requires a declaration")?,
                    )?);
                }
                "--persistence-root" => {
                    persistence_root = Some(PathBuf::from(
                        values.next().ok_or("--persistence-root requires a value")?,
                    ))
                }
                "--content-store-root" => {
                    content_store_root = Some(PathBuf::from(
                        values
                            .next()
                            .ok_or("--content-store-root requires a value")?,
                    ))
                }
                "--exercise" => exercise = true,
                "--performance-probe" => {
                    let iterations = values
                        .next()
                        .ok_or("--performance-probe requires an iteration count")?
                        .parse::<u32>()
                        .map_err(|_| "--performance-probe must be an integer in 1..=256")?;
                    if iterations == 0 || iterations > 256 {
                        return Err("--performance-probe must be in 1..=256".to_owned());
                    }
                    performance_probe = Some(iterations);
                }
                "--supervised" => supervised = true,
                "--debugger" => debugger = true,
                "--worker" => worker = true,
                "--worker-channel" => {
                    worker_channel = Some(
                        values
                            .next()
                            .ok_or("--worker-channel requires a loopback socket address")?
                            .parse()
                            .map_err(|_| "--worker-channel must be a socket address")?,
                    )
                }
                "--runtime-instance-id" => {
                    let value = values
                        .next()
                        .ok_or("--runtime-instance-id requires a nonzero u64")?
                        .parse::<u64>()
                        .map_err(|_| "--runtime-instance-id must be a nonzero u64")?;
                    if value == 0 {
                        return Err("--runtime-instance-id must be a nonzero u64".to_owned());
                    }
                    runtime_instance_id = Some(RuntimeInstanceId::new(value));
                }
                "--help" => {
                    return Err(format!(
                        "usage: rusty-product-host --product <Product-directory> --loader <nativeaot|coreclr> [--supervised] [--debugger] [--runtime-instance-id <nonzero-u64>] [--persistence-root <absolute-path>] [--content-store-root <absolute-path>] [--exercise] [--performance-probe <1..=256>]\n\nThe Product directory contains product.json plus its declared managed/native artifacts, UI, and admitted content. The matched Engine browser shell is discovered beside this runtime-pack binary; Product directories never carry Engine JavaScript. `--loader` chooses one exact optional manifest artifact. `--supervised` is the explicit rusty-dev stdin-close shutdown hook. `--debugger` disables worker startup/callback deadlines for supervised CoreCLR debugging; normal sessions retain their deadlines. `--runtime-instance-id` names this host-owned runtime incarnation; direct launches allocate a process-local fallback when it is omitted. Server bind/port and explicit liveDebug opt-in are Product metadata. `--identity` prints machine-readable matched runtime identity; `--version` prints a concise diagnostic identity.\n\n{PHYSICAL_MAPPING_USAGE}"
                    ));
                }
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }
        let staged_loader = staged_launch
            .as_deref()
            .map(StagedLaunchInput::read)
            .transpose()?;
        if loader.is_some() && staged_loader.is_some() {
            return Err("--loader and --staged-launch are mutually exclusive".to_owned());
        }
        let product_path = product.clone();
        let product = product.map(|root| ProductBundle::read(&root)).transpose()?;
        if product.is_some()
            && (library.is_some()
                || runtime_config_path.is_some()
                || bundle_dir.is_some()
                || content_dir.is_some()
                || mode.is_some()
                || staged_launch.is_some()
                || port != 0
                || bind_host != Ipv4Addr::LOCALHOST
                || live_debug
                || !direct_intents.is_empty()
                || !physical_mappings.is_empty())
        {
            return Err("--product is the canonical bundle launch and cannot be combined with legacy product arguments".to_owned());
        }
        let loader = match (product.as_ref(), loader.or(staged_loader)) {
            (Some(_), Some(loader)) => loader,
            (Some(_), None) => {
                return Err(
                    "--loader is required with --product; choose nativeaot or coreclr".to_owned(),
                );
            }
            (None, Some(loader)) => loader,
            (None, None) => ProductLoader::NativeAot,
        };
        let arguments = Self {
            loader,
            product,
            product_path,
            library,
            runtime_config_path,
            bundle_dir,
            content_dir,
            port,
            bind_host,
            mode,
            direct_intents,
            physical_mappings,
            persistence_root,
            content_store_root,
            legacy_live_debug: live_debug,
            exercise,
            performance_probe,
            supervised,
            debugger,
            runtime_instance_id,
            worker,
            worker_channel,
        };
        if arguments.debugger
            && (!matches!(arguments.loader, ProductLoader::CoreClr)
                || !(arguments.supervised || arguments.worker))
        {
            return Err("--debugger requires supervised CoreCLR (rusty dev --debugger)".to_owned());
        }
        if arguments.worker != arguments.worker_channel.is_some() {
            return Err("--worker and --worker-channel must be supplied together".to_owned());
        }
        if arguments.exercise && arguments.performance_probe.is_some() {
            return Err("--exercise and --performance-probe are mutually exclusive".to_owned());
        }
        let is_demand = match &arguments.product {
            Some(product) => matches!(
                product.lifecycle,
                runtime_lifecycle::RuntimeLifecycleConfig::Demand
            ),
            None => matches!(arguments.mode, Some(RuntimeMode::Demand)),
        };
        if arguments.performance_probe.is_some() && !is_demand {
            return Err("--performance-probe requires --mode demand".to_owned());
        }
        if let Some(product) = &arguments.product {
            product.selected_artifacts(arguments.loader)?;
        } else {
            if arguments.library.is_none()
                || arguments.bundle_dir.is_none()
                || arguments.content_dir.is_none()
                || arguments.mode.is_none()
            {
                return Err("legacy launch requires --library, --bundle-dir, --content-dir, and --mode (or use --product)".to_owned());
            }
            match (arguments.loader, &arguments.runtime_config_path) {
                (ProductLoader::CoreClr, None) => {
                    return Err(
                        "--loader coreclr requires --runtimeconfig <product.runtimeconfig.json>"
                            .to_owned(),
                    );
                }
                (ProductLoader::NativeAot, Some(_)) => {
                    return Err("--runtimeconfig is only valid with --loader coreclr".to_owned());
                }
                _ => {}
            }
            CompiledInputMappings::standard(
                arguments.direct_intents.clone(),
                arguments.physical_mappings.clone(),
            )
            .map_err(|error| format!("--physical-mapping configuration is invalid: {error}"))?;
        }
        Ok(arguments)
    }
}

fn next_direct_runtime_instance_id() -> RuntimeInstanceId {
    // Direct host invocation does not have a long-lived `rusty dev`
    // supervisor. Give each in-process launch a nonzero, process-local
    // incarnation instead of reviving the old fixed identity.
    let ordinal = NEXT_DIRECT_RUNTIME_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    let process_seed = u64::from(std::process::id()).max(1);
    RuntimeInstanceId::new(process_seed.saturating_add(ordinal).max(1))
}

fn parse_direct_intent(value: &str) -> Result<DirectInputIntentDescriptor, String> {
    let (id, value_kind) = value
        .split_once('=')
        .ok_or("--direct-intent requires id=digital, id=axis, or id=payload:contract")?;
    if let Some(contract) = value_kind.strip_prefix("payload:") {
        return DirectInputIntentDescriptor::product_payload(id, contract)
            .map_err(|error| error.to_string());
    }
    let value_kind = match value_kind {
        "digital" => IntentValueKind::Digital,
        "axis" => IntentValueKind::Axis,
        _ => return Err("--direct-intent value kind must be digital or axis".to_owned()),
    };
    DirectInputIntentDescriptor::new(id, value_kind).map_err(|error| error.to_string())
}

fn parse_physical_mapping(value: &str) -> Result<RuntimeInputMapping, String> {
    let (mapping_id, declaration) = value
        .split_once('=')
        .ok_or("--physical-mapping requires mapping-id=intent-id:<trigger>")?;
    let mut tokens = declaration.split(':');
    let intent = tokens
        .next()
        .ok_or("--physical-mapping requires an intent id")?;
    let trigger_kind = tokens
        .next()
        .ok_or("--physical-mapping requires a trigger kind")?;
    let trigger = match trigger_kind {
        "key" => {
            let code =
                parse_keyboard_control(next_mapping_token(&mut tokens, "keyboard control")?)?;
            let edge = parse_input_edge(next_mapping_token(&mut tokens, "key edge")?)?;
            let (context, chord) = parse_mapping_qualifiers(&mut tokens, true)?;
            RuntimeInputTrigger::Key {
                code,
                edge,
                chord,
                context,
            }
        }
        "pointer-button" => RuntimeInputTrigger::PointerButton {
            button: parse_pointer_button(next_mapping_token(&mut tokens, "pointer button")?)?,
            edge: parse_input_edge(next_mapping_token(&mut tokens, "pointer-button edge")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "pointer-axis" => RuntimeInputTrigger::PointerAxis {
            axis: parse_input_axis(next_mapping_token(&mut tokens, "pointer axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "wheel" => RuntimeInputTrigger::Wheel {
            axis: parse_input_axis(next_mapping_token(&mut tokens, "wheel axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "controller-button" => RuntimeInputTrigger::ControllerButton {
            button: parse_controller_button(next_mapping_token(&mut tokens, "controller button")?)?,
            edge: parse_input_edge(next_mapping_token(&mut tokens, "controller-button edge")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "controller-axis" => RuntimeInputTrigger::ControllerAxis {
            axis: parse_controller_axis(next_mapping_token(&mut tokens, "controller axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        _ => {
            return Err(format!(
                "--physical-mapping trigger `{trigger_kind}` is unsupported; expected key, pointer-button, pointer-axis, wheel, controller-button, or controller-axis"
            ));
        }
    };
    RuntimeInputMapping::new(mapping_id, intent, trigger)
        .map_err(|error| format!("--physical-mapping declaration is invalid: {error}"))
}

fn next_mapping_token<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    expected: &str,
) -> Result<&'a str, String> {
    tokens
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("--physical-mapping requires a {expected}"))
}

fn parse_mapping_qualifiers<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    allows_chord: bool,
) -> Result<(Option<InputContext>, Vec<KeyboardControl>), String> {
    let mut context = None;
    let mut chord = Vec::new();
    for qualifier in tokens {
        if let Some(value) = qualifier.strip_prefix("context=") {
            if context.is_some() {
                return Err("--physical-mapping may declare context only once".to_owned());
            }
            context = Some(
                InputContext::new(value)
                    .map_err(|error| format!("--physical-mapping context is invalid: {error}"))?,
            );
        } else if let Some(value) = qualifier.strip_prefix("chord=") {
            if !allows_chord {
                return Err(
                    "--physical-mapping chord is supported only for key triggers".to_owned(),
                );
            }
            if !chord.is_empty() {
                return Err("--physical-mapping may declare chord only once".to_owned());
            }
            chord = parse_chord(value)?;
        } else {
            return Err(format!(
                "--physical-mapping qualifier `{qualifier}` is unsupported"
            ));
        }
    }
    Ok((context, chord))
}

fn parse_chord(value: &str) -> Result<Vec<KeyboardControl>, String> {
    let controls = value
        .split('+')
        .map(parse_keyboard_control)
        .collect::<Result<Vec<_>, _>>()?;
    if controls.is_empty() || controls.len() > MAX_MAPPING_CHORD_CONTROLS {
        return Err(format!(
            "--physical-mapping chord must contain 1 to {MAX_MAPPING_CHORD_CONTROLS} keyboard controls"
        ));
    }
    if (1..controls.len()).any(|index| controls[..index].contains(&controls[index])) {
        return Err("--physical-mapping chord controls must be unique".to_owned());
    }
    Ok(controls)
}

fn parse_input_edge(value: &str) -> Result<InputEdge, String> {
    match value {
        "held" => Ok(InputEdge::Held),
        "pressed" => Ok(InputEdge::Pressed),
        "released" => Ok(InputEdge::Released),
        _ => Err(format!("--physical-mapping edge `{value}` is unsupported")),
    }
}

fn parse_keyboard_control(value: &str) -> Result<KeyboardControl, String> {
    let control = match value {
        "key-a" => KeyboardControl::KeyA,
        "key-b" => KeyboardControl::KeyB,
        "key-c" => KeyboardControl::KeyC,
        "key-d" => KeyboardControl::KeyD,
        "key-e" => KeyboardControl::KeyE,
        "key-f" => KeyboardControl::KeyF,
        "key-g" => KeyboardControl::KeyG,
        "key-h" => KeyboardControl::KeyH,
        "key-i" => KeyboardControl::KeyI,
        "key-j" => KeyboardControl::KeyJ,
        "key-k" => KeyboardControl::KeyK,
        "key-l" => KeyboardControl::KeyL,
        "key-m" => KeyboardControl::KeyM,
        "key-n" => KeyboardControl::KeyN,
        "key-o" => KeyboardControl::KeyO,
        "key-p" => KeyboardControl::KeyP,
        "key-q" => KeyboardControl::KeyQ,
        "key-r" => KeyboardControl::KeyR,
        "key-s" => KeyboardControl::KeyS,
        "key-t" => KeyboardControl::KeyT,
        "key-u" => KeyboardControl::KeyU,
        "key-v" => KeyboardControl::KeyV,
        "key-w" => KeyboardControl::KeyW,
        "key-x" => KeyboardControl::KeyX,
        "key-y" => KeyboardControl::KeyY,
        "key-z" => KeyboardControl::KeyZ,
        "digit-0" => KeyboardControl::Digit0,
        "digit-1" => KeyboardControl::Digit1,
        "digit-2" => KeyboardControl::Digit2,
        "digit-3" => KeyboardControl::Digit3,
        "digit-4" => KeyboardControl::Digit4,
        "digit-5" => KeyboardControl::Digit5,
        "digit-6" => KeyboardControl::Digit6,
        "digit-7" => KeyboardControl::Digit7,
        "digit-8" => KeyboardControl::Digit8,
        "digit-9" => KeyboardControl::Digit9,
        "space" => KeyboardControl::Space,
        "enter" => KeyboardControl::Enter,
        "escape" => KeyboardControl::Escape,
        "shift-left" => KeyboardControl::ShiftLeft,
        "shift-right" => KeyboardControl::ShiftRight,
        "control-left" => KeyboardControl::ControlLeft,
        "control-right" => KeyboardControl::ControlRight,
        "alt-left" => KeyboardControl::AltLeft,
        "alt-right" => KeyboardControl::AltRight,
        _ => {
            return Err(format!(
                "--physical-mapping keyboard control `{value}` is unsupported"
            ));
        }
    };
    Ok(control)
}

fn parse_pointer_button(value: &str) -> Result<PointerButton, String> {
    match value {
        "primary" => Ok(PointerButton::Primary),
        "secondary" => Ok(PointerButton::Secondary),
        "middle" => Ok(PointerButton::Middle),
        _ => Err(format!(
            "--physical-mapping pointer button `{value}` is unsupported"
        )),
    }
}

fn parse_input_axis(value: &str) -> Result<InputAxis, String> {
    match value {
        "x" => Ok(InputAxis::X),
        "y" => Ok(InputAxis::Y),
        _ => Err(format!("--physical-mapping axis `{value}` is unsupported")),
    }
}

fn parse_controller_button(value: &str) -> Result<ControllerButton, String> {
    match value {
        "button-0" => Ok(ControllerButton::Button0),
        "button-1" => Ok(ControllerButton::Button1),
        "button-2" => Ok(ControllerButton::Button2),
        "button-3" => Ok(ControllerButton::Button3),
        "button-4" => Ok(ControllerButton::Button4),
        "button-5" => Ok(ControllerButton::Button5),
        "button-6" => Ok(ControllerButton::Button6),
        "button-7" => Ok(ControllerButton::Button7),
        "button-8" => Ok(ControllerButton::Button8),
        "button-9" => Ok(ControllerButton::Button9),
        "button-10" => Ok(ControllerButton::Button10),
        "button-11" => Ok(ControllerButton::Button11),
        "button-12" => Ok(ControllerButton::Button12),
        "button-13" => Ok(ControllerButton::Button13),
        "button-14" => Ok(ControllerButton::Button14),
        "button-15" => Ok(ControllerButton::Button15),
        _ => Err(format!(
            "--physical-mapping controller button `{value}` is unsupported"
        )),
    }
}

fn parse_controller_axis(value: &str) -> Result<ControllerAxis, String> {
    match value {
        "axis-0" => Ok(ControllerAxis::Axis0),
        "axis-1" => Ok(ControllerAxis::Axis1),
        "axis-2" => Ok(ControllerAxis::Axis2),
        "axis-3" => Ok(ControllerAxis::Axis3),
        _ => Err(format!(
            "--physical-mapping controller axis `{value}` is unsupported"
        )),
    }
}

fn runtime_browser_root() -> Result<PathBuf, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("could not locate runtime-pack executable: {error}"))?;
    let pack_root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("runtime-pack executable has no pack root")?;
    let browser = pack_root.join("share/browser");
    if !browser.is_dir() {
        return Err(format!(
            "matched runtime browser shell is missing at `{}`; launch through a runtime pack",
            browser.display()
        ));
    }
    Ok(browser)
}

fn load_bundle(
    root: &Path,
    product: &ProductBundle,
    render_resources: &[ProductDevRendererResource],
) -> Result<ProductDevBundle, String> {
    let mut entries = Vec::new();
    collect_bundle(root, root, &mut entries)?;
    entries.extend(product.browser_entries(render_resources)?);
    ProductDevBundle::new(entries).map_err(|error| error.to_string())
}

fn load_legacy_bundle(
    root: &Path,
    render_resources: &[ProductDevRendererResource],
) -> Result<ProductDevBundle, String> {
    let mut entries = Vec::new();
    collect_bundle(root, root, &mut entries)?;
    entries.extend(
        product_dev_host::product_dev_renderer_preload_entries(render_resources)
            .map_err(|error| error.to_string())?,
    );
    ProductDevBundle::new(entries).map_err(|error| error.to_string())
}

fn collect_bundle(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ProductDevBundleEntry>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_bundle(root, &path, entries)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "bundle entry escaped root")?
                .to_string_lossy()
                .replace('\\', "/");
            let content_type = content_type(&relative)
                .ok_or_else(|| format!("bundle file `{relative}` has no admitted content type"))?;
            entries.push(
                ProductDevBundleEntry::new(
                    relative,
                    content_type,
                    fs::read(path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(())
}

fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()? {
        "html" => Some("text/html; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "json" => Some("application/json; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_proxy_forwards_input_recovery_fences_and_their_fresh_binding() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut worker = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (writer, _) = listener.accept().unwrap();
        let (responses, response_rx) = mpsc::channel();
        let (outputs, _) = mpsc::sync_channel(8);
        let (diagnostics, _) = mpsc::sync_channel(8);
        let (failures, _) = mpsc::sync_channel(8);
        let mut proxy = WorkerRuntime {
            connection: Arc::new(Mutex::new(WorkerConnection {
                child: Command::new("sleep").arg("30").spawn().unwrap(),
                writer,
                responses: response_rx,
                next_request_id: 1,
                operation_timeout: Some(Duration::from_secs(1)),
                pending_attribution: None,
                terminal_cause: WorkerTerminalCause::default(),
                retiring: Arc::new(AtomicBool::new(false)),
                reader: None,
                generation: 1,
            })),
            outputs,
            output_generation: Arc::new(AtomicUsize::new(1)),
            diagnostics,
            failures,
            scheduler_inflight: Arc::new(Mutex::new(None)),
            operation_timeout: Some(Duration::from_secs(1)),
        };
        let mut binding = ProductDevRuntimeBinding {
            instance_id: CanonicalU64::new(7),
            generation: CanonicalU64::new(1),
            control_revision: CanonicalU64::new(1),
        };
        for (index, operation) in [
            Some(ProductDevControlOperation::Replace),
            Some(ProductDevControlOperation::Release),
            None, // Shell recovery uses the child's current binding, not a caller's stale one.
        ]
        .into_iter()
        .enumerate()
        {
            let fresh = ProductDevRuntimeBinding {
                control_revision: CanonicalU64::new(index as u64 + 2),
                ..binding
            };
            responses.send(Ok(WorkerResponse {
                response: ProductDevWorkerResponse {
                    request_id: index as u64 + 1,
                    attribution: None,
                    result: Some(serde_json::json!({
                        "accepted": true, "code": "DEV_HOST_ACCEPTED", "disposition": "accepted",
                        "operation": operation.unwrap_or(ProductDevControlOperation::Replace).operation_kind(), "binding": fresh, "nextInputSequence": "1",
                    })),
                    outputs: vec![serde_json::json!({
                        "kind": "binding", "runtime": fresh, "nextInputSequence": "1",
                    })],
                    error: None,
                },
                connection_output_cursor: None,
            })).unwrap();
            let receipt = match operation {
                Some(operation) => proxy.control(operation, binding),
                None => proxy.recover_input_overflow(),
            }
            .expect("worker input recovery fence is supported");
            let request: ProductDevWorkerRequest = read_worker_frame(&mut worker).unwrap();
            assert_eq!(worker_request_id(&request), index as u64 + 1);
            match (operation, request) {
                (
                    Some(expected),
                    ProductDevWorkerRequest::Control {
                        operation,
                        binding: observed,
                        ..
                    },
                ) => {
                    assert_eq!(worker_control(operation), expected);
                    assert_eq!(observed, binding);
                }
                (None, ProductDevWorkerRequest::RecoverInput { .. }) => {}
                _ => panic!(
                    "the proxy must forward input recovery rather than restarting the product"
                ),
            }
            assert!(receipt.result().is_accepted());
            assert_eq!(receipt.result().binding(), Some(fresh));
            assert_eq!(
                receipt.result().next_input_sequence(),
                Some(CanonicalU64::new(1))
            );
            assert_eq!(receipt.into_parts().1.len(), 1);
            binding = fresh;
        }
    }

    #[test]
    fn worker_output_group_only_admits_large_complete_baselines() {
        let runtime = serde_json::json!({
            "instanceId": "7",
            "generation": "3",
            "controlRevision": "5",
        });
        let frame = serde_json::json!({
            "kind": "frame",
            "frame": { "payload": "x".repeat(product_dev_host::MAX_OUTPUT_AGGREGATE_BYTES + 1) },
        });
        let ordinary = worker_outputs(vec![frame.clone()])
            .expect_err("ordinary worker output keeps the 16 MiB limit");
        assert_eq!(ordinary.code(), "DEV_HOST_OUTPUT_BOUNDS");

        worker_outputs(vec![
            serde_json::json!({
                "kind": "binding",
                "runtime": runtime,
                "nextInputSequence": "0",
            }),
            frame,
            serde_json::json!({
                "kind": "complete-baseline",
                "runtime": runtime,
                "publicationFrontiers": [],
            }),
            serde_json::json!({ "kind": "runtime-progress", "owner": "rust-host" }),
        ])
        .expect("worker admits a bounded baseline followed by ordinary tick output");
    }

    #[test]
    fn connection_response_keeps_its_ordered_output_boundary() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut child = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (channel, _) = listener.accept().unwrap();
        let (response_tx, response_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::sync_channel(8);
        let (diagnostic_tx, _diagnostic_rx) = mpsc::sync_channel(8);
        let (failure_tx, _failure_rx) = mpsc::sync_channel(8);
        let reader = thread::spawn(move || {
            worker_reader(
                channel,
                response_tx,
                output_tx,
                diagnostic_tx,
                failure_tx,
                Arc::new(Mutex::new(None)),
                WorkerReaderLifetime {
                    generation: 3,
                    retiring: Arc::new(AtomicBool::new(false)),
                    terminal_cause: WorkerTerminalCause::default(),
                },
            )
        });
        let output = ProductDevWorkerEvent::Outputs {
            outputs: vec![serde_json::json!({ "kind": "runtime-progress", "owner": "rust-host" })],
        };
        write_worker_frame(&mut child, &output).unwrap();
        write_worker_frame(
            &mut child,
            &ProductDevWorkerEvent::ConnectionResponse(ProductDevWorkerResponse {
                attribution: None,
                request_id: 7,
                result: Some(serde_json::json!({ "accepted": true })),
                outputs: Vec::new(),
                error: None,
            }),
        )
        .unwrap();
        write_worker_frame(&mut child, &output).unwrap();
        assert!(matches!(
            output_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            ProductDevWorkerPublication::Outputs(_)
        ));
        let ProductDevWorkerPublication::ConnectionBoundary {
            generation,
            acknowledged,
        } = output_rx.recv_timeout(Duration::from_secs(2)).unwrap()
        else {
            panic!("the snapshot boundary precedes later output");
        };
        assert_eq!(generation, 3);
        assert!(matches!(
            response_rx.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        acknowledged.send(Some(41)).unwrap();
        let response = response_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert_eq!(response.response.request_id, 7);
        assert_eq!(response.connection_output_cursor, Some(41));
        assert!(matches!(
            output_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            ProductDevWorkerPublication::Outputs(_)
        ));
        child.shutdown(Shutdown::Both).unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn callback_failure_survives_eof_and_later_connection_writes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let child_channel = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (channel, _) = listener.accept().unwrap();
        let mut writer = channel.try_clone().unwrap();
        let (response_tx, response_rx) = mpsc::channel();
        let (output_tx, _) = mpsc::sync_channel(8);
        let (diagnostic_tx, _diagnostic_rx) = mpsc::sync_channel(8);
        let (failure_tx, _failure_rx) = mpsc::sync_channel(8);
        let terminal_cause = WorkerTerminalCause::default();
        let reader_cause = terminal_cause.clone();
        let reader_failure = failure_tx.clone();
        let reader = thread::spawn(move || {
            worker_reader(
                channel,
                response_tx,
                output_tx,
                diagnostic_tx,
                reader_failure,
                Arc::new(Mutex::new(None)),
                WorkerReaderLifetime {
                    generation: 1,
                    retiring: Arc::new(AtomicBool::new(false)),
                    terminal_cause: reader_cause,
                },
            )
        });
        let original = worker_runtime_error(
            "CSHARP_PRODUCT_CALL",
            "Look request rejected: DeltaLimitExceeded",
        );
        report_scheduler_failure(
            &Arc::new(Mutex::new(child_channel)),
            Vec::new(),
            original.clone(),
        );
        reader.join().unwrap();
        writer.shutdown(Shutdown::Both).unwrap();
        assert!(write_worker_frame(
            &mut writer,
            &ProductDevWorkerRequest::Activate { request_id: 1 }
        )
        .is_err());
        let mut connection = WorkerConnection {
            child: Command::new("sleep").arg("30").spawn().unwrap(),
            writer,
            responses: response_rx,
            next_request_id: 1,
            operation_timeout: Some(Duration::from_secs(1)),
            pending_attribution: None,
            terminal_cause,
            retiring: Arc::new(AtomicBool::new(false)),
            reader: None,
            generation: 1,
        };
        for _ in 0..2 {
            let result = invoke_connection::<serde_json::Value>(
                &failure_tx,
                &mut connection,
                |request_id| ProductDevWorkerRequest::Activate { request_id },
            );
            let error = result.expect_err("retired worker must remain unavailable");
            assert_eq!(error.code(), original.code());
            assert_eq!(error.diagnostic(), original.diagnostic());
            assert_eq!(error.recovery(), original.recovery());
        }
        // spawn_connection creates a new holder for every replacement.
        assert!(WorkerTerminalCause::default().read().is_none());
    }

    #[test]
    fn reader_distinguishes_deliberate_retirement_from_unexpected_eof() {
        for retiring in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let child = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (channel, _) = listener.accept().unwrap();
            let (response_tx, _) = mpsc::channel();
            let (output_tx, _) = mpsc::sync_channel(8);
            let (diagnostic_tx, diagnostic_rx) = mpsc::sync_channel(8);
            let (failure_tx, failure_rx) = mpsc::sync_channel(8);
            drop(child);
            worker_reader(
                channel,
                response_tx,
                output_tx,
                diagnostic_tx,
                failure_tx,
                Arc::new(Mutex::new(None)),
                WorkerReaderLifetime {
                    generation: 4,
                    retiring: Arc::new(AtomicBool::new(retiring)),
                    terminal_cause: WorkerTerminalCause::default(),
                },
            );
            assert_eq!(diagnostic_rx.try_recv().is_ok(), !retiring);
            assert_eq!(failure_rx.try_recv().is_ok(), !retiring);
        }
    }

    fn parse_test_args(arguments: &[&str]) -> Result<Arguments, String> {
        let mut values = vec![
            "--library".to_owned(),
            "product.so".to_owned(),
            "--bundle-dir".to_owned(),
            "bundle".to_owned(),
            "--content-dir".to_owned(),
            "content".to_owned(),
            "--mode".to_owned(),
            "demand".to_owned(),
        ];
        values.extend(arguments.iter().map(|value| (*value).to_owned()));
        Arguments::parse_from(values)
    }

    fn parse_test_error(arguments: &[&str]) -> String {
        match parse_test_args(arguments) {
            Ok(_) => panic!("argument list unexpectedly parsed"),
            Err(error) => error,
        }
    }

    #[test]
    fn debugger_wait_is_explicit_and_coreclr_scoped() {
        let normal = parse_test_args(&[]).expect("normal arguments");
        assert_eq!(
            normal.worker_operation_timeout(),
            Some(Duration::from_secs(5))
        );
        assert!(parse_test_error(&["--debugger", "--supervised"])
            .contains("requires supervised CoreCLR"));
        assert!(parse_test_error(&[
            "--debugger",
            "--loader",
            "coreclr",
            "--runtimeconfig",
            "product.runtimeconfig.json"
        ])
        .contains("requires supervised CoreCLR"));
        let mut debugging = parse_test_args(&[
            "--debugger",
            "--supervised",
            "--loader",
            "coreclr",
            "--runtimeconfig",
            "product.runtimeconfig.json",
            "--runtime-instance-id",
            "7",
        ])
        .expect("explicit debugger mode");
        assert_eq!(debugging.worker_operation_timeout(), None);
        debugging.product_path = Some(PathBuf::from("/product"));
        let worker =
            worker_arguments(&debugging, "127.0.0.1:1".parse().unwrap()).expect("worker arguments");
        assert!(worker.iter().any(|argument| argument == "--debugger"));
    }

    #[test]
    fn normal_launch_preserves_wasd_mapping_declaration_order() {
        let args = parse_test_args(&[
            "--direct-intent",
            "move.forward=digital",
            "--direct-intent",
            "move.left=digital",
            "--direct-intent",
            "move.backward=digital",
            "--direct-intent",
            "move.right=digital",
            "--physical-mapping",
            "move-forward=move.forward:key:key-w:held",
            "--physical-mapping",
            "move-left=move.left:key:key-a:held",
            "--physical-mapping",
            "move-backward=move.backward:key:key-s:held",
            "--physical-mapping",
            "move-right=move.right:key:key-d:held",
        ])
        .expect("normal mapping declaration parses");

        assert!(!args.exercise);
        assert!(matches!(args.loader, ProductLoader::NativeAot));
        let (intents, mappings) = args.input_configuration();
        assert_eq!(
            mappings
                .iter()
                .map(RuntimeInputMapping::id)
                .collect::<Vec<_>>(),
            ["move-forward", "move-left", "move-backward", "move-right"]
        );
        assert!(CompiledInputMappings::standard(intents, mappings).is_ok());
    }

    #[test]
    fn parser_supports_every_typed_physical_trigger_family() {
        let args = parse_test_args(&[
            "--direct-intent",
            "key=digital",
            "--direct-intent",
            "pointer-button=digital",
            "--direct-intent",
            "pointer-axis=axis",
            "--direct-intent",
            "wheel=axis",
            "--direct-intent",
            "controller-button=digital",
            "--direct-intent",
            "controller-axis=axis",
            "--physical-mapping",
            "key=key:key:key-w:pressed",
            "--physical-mapping",
            "pointer-button=pointer-button:pointer-button:primary:released",
            "--physical-mapping",
            "pointer-axis=pointer-axis:pointer-axis:x",
            "--physical-mapping",
            "wheel=wheel:wheel:y",
            "--physical-mapping",
            "controller-button=controller-button:controller-button:button-0:held",
            "--physical-mapping",
            "controller-axis=controller-axis:controller-axis:axis-3",
        ])
        .expect("all supported trigger families parse");
        let (_, mappings) = args.input_configuration();

        assert!(matches!(
            mappings[0].trigger(),
            RuntimeInputTrigger::Key { .. }
        ));
        assert!(matches!(
            mappings[1].trigger(),
            RuntimeInputTrigger::PointerButton { .. }
        ));
        assert!(matches!(
            mappings[2].trigger(),
            RuntimeInputTrigger::PointerAxis { .. }
        ));
        assert!(matches!(
            mappings[3].trigger(),
            RuntimeInputTrigger::Wheel { .. }
        ));
        assert!(matches!(
            mappings[4].trigger(),
            RuntimeInputTrigger::ControllerButton { .. }
        ));
        assert!(matches!(
            mappings[5].trigger(),
            RuntimeInputTrigger::ControllerAxis { .. }
        ));
    }

    #[test]
    fn parser_admits_key_context_and_chord() {
        let args = parse_test_args(&[
            "--direct-intent",
            "editor.save=digital",
            "--physical-mapping",
            "save=editor.save:key:key-s:pressed:context=editor.text:chord=control-left+shift-left",
        ])
        .expect("contextual chord parses");
        let (_, mappings) = args.input_configuration();
        let RuntimeInputTrigger::Key {
            code,
            edge,
            chord,
            context,
        } = mappings[0].trigger()
        else {
            panic!("expected key trigger");
        };
        assert_eq!(*code, KeyboardControl::KeyS);
        assert_eq!(*edge, InputEdge::Pressed);
        assert_eq!(
            chord,
            &[KeyboardControl::ControlLeft, KeyboardControl::ShiftLeft]
        );
        assert_eq!(
            context.as_ref().map(InputContext::as_str),
            Some("editor.text")
        );
    }

    #[test]
    fn parser_rejects_malformed_unknown_duplicate_and_mismatched_mappings() {
        for declaration in [
            "move=move.forward:key:key-w",
            "move=move.forward:gesture:key-w:held",
            "move=move.forward:key:not-a-key:held",
            "move=move.forward:pointer-axis:x:chord=control-left",
        ] {
            let error = parse_test_error(&[
                "--direct-intent",
                "move.forward=digital",
                "--physical-mapping",
                declaration,
            ]);
            assert!(error.contains("--physical-mapping"));
        }

        let unknown = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.missing:key:key-w:held",
        ]);
        assert!(unknown.contains("UnknownIntent"));

        let duplicate = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.forward:key:key-w:held",
            "--physical-mapping",
            "move=move.forward:key:key-a:held",
        ]);
        assert!(duplicate.contains("DuplicateMapping"));

        let mismatch = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.forward:pointer-axis:x",
        ]);
        assert!(mismatch.contains("IntentValueKindMismatch"));
    }

    #[test]
    fn parser_enforces_mapping_and_chord_declaration_bounds() {
        let mut values = vec!["--direct-intent", "move.forward=digital"];
        for _ in 0..=MAX_PHYSICAL_MAPPINGS {
            values.extend(["--physical-mapping", "move=move.forward:key:key-w:held"]);
        }
        let error = parse_test_error(&values);
        assert!(error.contains("at most 256"));

        let chord = (0..=MAX_MAPPING_CHORD_CONTROLS)
            .map(|_| "key-w")
            .collect::<Vec<_>>()
            .join("+");
        let declaration = format!("move=move.forward:key:key-w:held:chord={chord}");
        let error = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            &declaration,
        ]);
        assert!(error.contains("1 to 8"));
    }

    #[test]
    fn help_documents_the_physical_mapping_vocabulary() {
        let error = parse_test_error(&["--help"]);
        assert!(error.contains(PHYSICAL_MAPPING_USAGE));
    }

    #[test]
    fn parser_requires_an_explicit_coreclr_runtimeconfig() {
        let missing = Arguments::parse_from(
            [
                "--loader",
                "coreclr",
                "--library",
                "product.dll",
                "--bundle-dir",
                "bundle",
                "--content-dir",
                "content",
                "--mode",
                "demand",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert!(missing
            .expect_err("CoreCLR without runtimeconfig is rejected")
            .contains("requires --runtimeconfig"));

        let native_with_runtimeconfig =
            parse_test_error(&["--runtimeconfig", "product.runtimeconfig.json"]);
        assert!(native_with_runtimeconfig.contains("only valid with --loader coreclr"));
    }

    #[test]
    fn staged_launch_input_selects_the_loader_without_defining_a_product_bundle() {
        let coreclr = serde_json::json!({
            "artifact": "rusty.product.runtime-launch",
            "schemaVersion": 1,
            "loader": "coreclr",
            "futureProductBundleField": { "ownedBy": 7699 },
        });
        let input: StagedLaunchInput =
            serde_json::from_value(coreclr).expect("forward-compatible launch input deserializes");
        assert_eq!(input.artifact, StagedLaunchInput::ARTIFACT);
        assert_eq!(input.schema_version, 1);
        assert!(matches!(
            ProductLoader::parse(&input.loader),
            Ok(ProductLoader::CoreClr)
        ));
    }

    #[test]
    fn identity_commands_are_exclusive_and_require_no_product_arguments() {
        assert!(matches!(
            Invocation::parse_from(["--identity".to_owned()]),
            Ok(Invocation::Identity {
                machine_readable: true
            })
        ));
        assert!(matches!(
            Invocation::parse_from(["--version".to_owned()]),
            Ok(Invocation::Identity {
                machine_readable: false
            })
        ));
    }

    #[test]
    fn parser_admits_the_explicit_supervised_shutdown_hook() {
        let args = parse_test_args(&["--supervised"])
            .expect("supervised shutdown hook parses for a foreground host");
        assert!(args.supervised);
    }

    #[test]
    fn parser_carries_a_supervisor_runtime_incarnation() {
        let args =
            parse_test_args(&["--runtime-instance-id", "77"]).expect("runtime incarnation parses");
        assert_eq!(
            args.runtime_instance_id
                .expect("configured runtime incarnation"),
            RuntimeInstanceId::new(77)
        );
        assert!(parse_test_error(&["--runtime-instance-id", "0"])
            .contains("--runtime-instance-id must be a nonzero u64"));
    }
}
