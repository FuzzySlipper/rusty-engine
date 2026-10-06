#[cfg(unix)]
use std::net::TcpListener;
use std::{
    env, fs,
    io::{BufRead, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

use csharp_product_runtime::{
    product_host_runtime_identity, CsharpProductContent, CsharpProductRuntime,
    CsharpProductRuntimeConfig,
};
#[cfg(unix)]
use product_host::ProductHostRuntime;
use product_host::{
    ProductHost, ProductHostAssetReload, ProductHostBundle, ProductHostBundleEntry,
    ProductHostConfig, ProductHostLog, ProductHostLogConfig, RunningProductHost,
};
use runtime_input::{
    CompiledInputMappings, ControllerAxis, ControllerButton, DirectInputIntentDescriptor,
    InputAxis, InputContext, InputEdge, IntentValueKind, KeyboardControl, PointerButton,
    RuntimeInputMapping, RuntimeInputTrigger,
};
use runtime_lifecycle::RuntimeInstanceId;

#[cfg(feature = "desktop")]
mod desktop;
#[cfg(unix)]
mod headless_browser;
mod product_bundle;
#[cfg(unix)]
mod supervisor;

/// Forwarded to a serving runtime after `rusty dev` restaged only UI or
/// bundle content: the runtime re-reads them without restarting the product.
/// A supervised runtime reads it on stdin on every platform.
pub(crate) const RELOAD_ASSETS_COMMAND: &str = "reload-assets";
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
  controller-button-value:<button-0..button-15>[:context=<identity>]\n\
  controller-axis:<axis-0..axis-3>[:context=<identity>]\n\
keyboard controls: key-a..key-z, digit-0..digit-9, space, enter, escape, shift-left,\n\
  shift-right, control-left, control-right, alt-left, alt-right,
  arrow-up, arrow-down, arrow-left, arrow-right";

/// CoreCLR and NativeAOT turn a managed null dereference (SIGSEGV) into a
/// `NullReferenceException` in a handler that runs on the thread's alternate
/// signal stack. They allocate that stack only when a thread has none. Rust
/// std gives every thread an 8 KiB alternate stack for its stack-overflow
/// message; the managed handler overflows it into std's guard page and the
/// process dies instead of throwing (#8753).
///
/// std installs its handler and those stacks only when SIGSEGV and SIGBUS
/// have their default action as it initializes, before `main`. They are
/// ignored for that window, and `main` restores the default first, so each
/// managed runtime allocates its own stacks. A Rust stack overflow in this
/// process ends as a plain SIGSEGV without std's message.
#[cfg(target_os = "linux")]
#[used]
#[link_section = ".init_array"]
static DEFER_FAULT_SIGNALS_TO_MANAGED_RUNTIME: extern "C" fn() = ignore_fault_signals;

#[cfg(target_os = "linux")]
extern "C" fn ignore_fault_signals() {
    set_fault_signal_action(libc::SIG_IGN);
}

#[cfg(target_os = "linux")]
fn set_fault_signal_action(action: libc::sighandler_t) {
    for signal in [libc::SIGSEGV, libc::SIGBUS] {
        // SAFETY: this only replaces the process-wide action; no handler of
        // ours runs. The kernel still kills on an ignored hardware fault.
        unsafe { libc::signal(signal, action) };
    }
}

fn main() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    set_fault_signal_action(libc::SIG_DFL);
    #[cfg(feature = "desktop")]
    desktop::run_chromium_subprocess()?;
    let mut args = match Invocation::parse()? {
        Invocation::Identity { machine_readable } => {
            print_runtime_identity(machine_readable);
            return Ok(());
        }
        Invocation::Launch(args) => args,
    };
    validate_headless_host(&args)?;
    #[cfg(unix)]
    if args.uses_supervisor() {
        let mut args = args;
        if args.runtime_instance_id.is_none() {
            // Direct launches own an incarnation too; rusty dev supplies one.
            args.runtime_instance_id = Some(next_direct_runtime_instance_id());
        }
        return supervisor::run(args);
    }
    // The window's device must exist before the product loads, so its
    // renderer is built on it.
    let render_output = args
        .product
        .as_ref()
        .map_or(csharp_product_runtime::RenderOutput::Stream, |product| {
            product.render_output
        });
    #[cfg(feature = "desktop")]
    let desktop = desktop::Desktop::open_if_selected(
        render_output,
        args.cef_dir.clone(),
        &args.cef_switches,
    )?;
    let diagnostics = ProductHostLog::new(args.log_config()).map_err(|error| error.to_string())?;
    let content = args.content().map_err(|error| error.to_string())?;
    let (library, runtimeconfig) = args.selected_artifacts()?;
    #[allow(unused_mut)]
    let mut runtime_config = args.runtime_config().with_diagnostics(diagnostics.clone());
    // Exercise and probe runs assert Engine behaviour and exit; they draw
    // nothing, so they need no GPU.
    if !args.exercise && args.performance_probe.is_none() {
        runtime_config = runtime_config.with_render_output(render_output);
    }
    #[cfg(feature = "desktop")]
    if let Some(desktop) = &desktop {
        runtime_config = runtime_config.with_window_gpu(desktop.gpu());
    }
    let mut runtime = match args.loader {
        ProductLoader::NativeAot => {
            CsharpProductRuntime::load_admitted(library, content, runtime_config)
        }
        ProductLoader::CoreClr => CsharpProductRuntime::load_coreclr_admitted(
            library,
            runtimeconfig.expect("CoreCLR Product manifest declares runtimeconfig"),
            content,
            runtime_config,
        ),
    }
    .map_err(|error| error.to_string())?;
    let bundle = match &args.product {
        Some(product) => load_bundle(&runtime_browser_root()?, product)?,
        None => load_legacy_bundle(args.bundle_dir.as_deref().expect("legacy bundle path"))?,
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
    let mut config = ProductHostConfig::new(args.port(), bundle.clone())
        .with_bind_host(args.bind_host())
        .with_live_debug(args.live_debug())
        .with_diagnostics(diagnostics)
        .with_ui_files(runtime.ui_files());
    config = config.with_presentation(runtime.presentation());
    if let Some(audio) = runtime.audio_stream() {
        config = config.with_audio_stream(audio);
    }
    if let Some(frames) = runtime.frame_stream() {
        config = config.with_frame_stream(frames);
    }
    if let Some(file) = &args.activity_file {
        config = config.with_activity(Arc::new(product_host::ProductHostActivity::new(
            file.clone(),
        )));
    }
    if let Some(capture) = runtime.frame_capture() {
        config = config.with_frame_capture(capture);
    }
    #[cfg(unix)]
    if let Some(fd) = args.serve_listener_fd {
        use std::os::fd::FromRawFd;
        // A supervised product runs from load, with or without a browser.
        let started = runtime
            .connect()
            .map_err(|error| format!("{}: {}", error.code(), error.diagnostic()))?;
        if !started.result().is_accepted() {
            return Err("CSHARP_START: the product did not start".to_owned());
        }
        // Serve only once the supervisor has stopped answering on the
        // shared listener; EOF instead means stop.
        print_line(supervisor::RUNTIME_READY_LINE);
        let mut command = String::new();
        let _ = std::io::stdin().lock().read_line(&mut command);
        if command.trim_end() != supervisor::SERVE_COMMAND {
            print_line("RUSTY_HOST shutdown={\"reason\":\"supervisor-stdin-closed\"}");
            return Ok(());
        }
        // The supervisor cleared close-on-exec so this process could inherit
        // the listener; set it again so nothing this process starts keeps
        // the port open.
        // SAFETY: fcntl on a descriptor this process owns.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(format!(
                "PRODUCT_HOST_BIND: could not keep the listener private: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the supervisor bound this listener and hands this process
        // sole use of the descriptor number.
        config = config.with_listener(unsafe { TcpListener::from_raw_fd(fd) });
    }
    #[cfg(feature = "desktop")]
    let window_scene = runtime.scene_driver().zip(runtime.window_timing());
    let host = ProductHost::start(runtime, config).map_err(|error| error.to_string())?;
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
            performance_summary(
                "csharp-rust-crossover",
                iterations,
                &durations,
                args.loader,
                args.product.as_ref(),
                args.performance_configuration.as_deref(),
            )
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
            performance_summary(
                "product-host-http",
                iterations,
                &host_durations,
                args.loader,
                args.product.as_ref(),
                args.performance_configuration.as_deref(),
            )
        );
        drop(output_stream);
        host.shutdown().map_err(|error| error.to_string())?;
    } else {
        let termination = install_termination_signal_hook();
        print_line(&format!(
            "C# {} product host listening at {}",
            args.loader.label(),
            host.origin()
        ));
        print_line("Press Ctrl+C to stop.");
        #[cfg(feature = "desktop")]
        let title = args.product.as_ref().map_or_else(
            || "Rusty Engine".to_owned(),
            |product| product.title.clone(),
        );
        // Only a runtime serving under the supervisor receives asset reloads.
        let reload_assets = args
            .serve_listener_fd
            .and(args.product.take())
            .map(|product| asset_reloader(host.asset_reload(), product));
        let supervised = args.supervised || args.serve_listener_fd.is_some();
        #[cfg(feature = "desktop")]
        if let (Some(desktop), Some((driver, timing))) = (desktop, window_scene) {
            // The window owns the main thread. The usual stop conditions
            // (signal, supervisor stdin, host stop) close it through the
            // shared flag, and closing it stops the host the same way.
            let origin = host.origin().to_string();
            return std::thread::scope(|scope| {
                let waiter = Arc::clone(&termination);
                let host = &host;
                scope.spawn(move || {
                    wait_for_process_termination(
                        supervised,
                        host,
                        Arc::clone(&waiter),
                        reload_assets,
                    );
                    waiter.store(true, Ordering::Relaxed);
                });
                desktop.run(
                    title,
                    &origin,
                    args.persistence_root.as_deref(),
                    driver,
                    timing,
                    Arc::clone(&termination),
                )
            });
        }
        wait_for_process_termination(supervised, &host, termination, reload_assets);
    }
    Ok(())
}

fn validate_headless_host(args: &Arguments) -> Result<(), String> {
    if !args.headless {
        return Ok(());
    }
    if args.product_path.is_none() {
        return Err("--headless requires a packaged --product bundle".to_owned());
    }
    // The supervisor opens and closes the headless page; it runs on Unix.
    if cfg!(not(unix)) {
        return Err("--headless is not supported on this platform".to_owned());
    }
    Ok(())
}

/// Writes one line to stdout. A supervised runtime can outlive a killed
/// supervisor's pipe; losing a status line must not abort product disposal.
fn print_line(line: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = writeln!(stdout, "{line}").and_then(|_| stdout.flush());
}

#[cfg(unix)]
fn browser_url(address: SocketAddr) -> String {
    let ip = if address.ip().is_unspecified() {
        Ipv4Addr::LOCALHOST.into()
    } else {
        address.ip()
    };
    format!("http://{ip}:{}", address.port())
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

/// Re-reads the staged UI into a fresh browser bundle and reloads the
/// runtime's content, keeping the old bundle and inventory on failure.
fn asset_reloader(reload: ProductHostAssetReload, product: ProductBundle) -> Box<dyn Fn() + Send> {
    Box::new(move || {
        let result = runtime_browser_root()
            .and_then(|root| load_bundle(&root, &product))
            .and_then(|bundle| {
                reload
                    .reload(bundle)
                    .map_err(|error| format!("{}: {}", error.code(), error.diagnostic()))
            });
        match result {
            Ok(()) => print_line("RUSTY_HOST assets-reloaded"),
            Err(error) => eprintln!("RUSTY_HOST PRODUCT_HOST_ASSET_RELOAD: {error}"),
        }
    })
}

/// The standard host is owned by its foreground process supervisor. In
/// particular, service launchers commonly provide a closed stdin, so EOF must
/// not be interpreted as a request to shut the host down. The `rusty dev`
/// supervisor opts in explicitly and retains the pipe while the child is live;
/// closing it is its cross-platform clean-replacement signal. Returning here
/// lets `RunningProductHost` and then `CsharpProductRuntime` execute their
/// normal shutdown/drop ordering before the process exits.
fn wait_for_process_termination(
    supervised: bool,
    host: &RunningProductHost,
    termination: Arc<AtomicBool>,
    reload_assets: Option<Box<dyn Fn() + Send>>,
) {
    if supervised {
        // Keep stdin as the supervisor's clean-stop mechanism, but read it on
        // a helper so a terminal runtime recovery can wake this foreground
        // owner without waiting for the supervisor to close the pipe. The
        // supervisor also forwards asset reloads here, one line each.
        let supervisor_stdin_closed = Arc::new(AtomicBool::new(false));
        let reader_closed = Arc::clone(&supervisor_stdin_closed);
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                if line == RELOAD_ASSETS_COMMAND {
                    if let Some(reload) = &reload_assets {
                        reload();
                    }
                }
            }
            reader_closed.store(true, Ordering::Release);
        });
        while !termination.load(Ordering::Relaxed)
            && !supervisor_stdin_closed.load(Ordering::Acquire)
            && !host.termination_requested()
        {
            std::thread::park_timeout(std::time::Duration::from_millis(50));
        }
        let reason = if host.termination_requested() {
            "host-stopped"
        } else if termination.load(Ordering::Relaxed) {
            "termination-signal"
        } else {
            "supervisor-stdin-closed"
        };
        print_line(&format!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}"));
    } else {
        while !termination.load(Ordering::Relaxed) && !host.termination_requested() {
            std::thread::park_timeout(std::time::Duration::from_millis(100));
        }
        let reason = if host.termination_requested() {
            "host-stopped"
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

fn performance_summary(
    lane: &str,
    iterations: u32,
    durations: &[u128],
    loader: ProductLoader,
    product: Option<&ProductBundle>,
    configuration: Option<&str>,
) -> serde_json::Value {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    let value_at = |fraction: f64| -> f64 {
        let index = ((sorted.len().saturating_sub(1)) as f64 * fraction).round() as usize;
        sorted[index] as f64 / 1_000_000.0
    };
    let mean = sorted.iter().copied().sum::<u128>() as f64 / sorted.len() as f64 / 1_000_000.0;
    let configuration = configuration
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("unspecified");
    let workload = product.map_or_else(
        || {
            serde_json::json!({
                "id": "legacy-source-launch",
                "version": 1,
                "configuration": configuration,
                "launch": "legacy",
                "loader": loader.identifier(),
            })
        },
        |product| {
            serde_json::json!({
                "id": product.id.as_str(),
                "version": 1,
                "configuration": configuration,
                "launch": "canonical-product-v1",
                "lifecycle": product.lifecycle_mode,
                "loader": loader.identifier(),
            })
        },
    );
    serde_json::json!({
        "schemaVersion": 1,
        "lane": lane,
        "loader": loader.identifier(),
        "runtime": loader.label(),
        "workload": workload,
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
    exercise: bool,
    performance_probe: Option<u32>,
    supervised: bool,
    debugger: bool,
    headless: bool,
    /// The Chromium executable `--headless` opens; else one on `PATH`.
    #[cfg_attr(not(unix), allow(dead_code))]
    chromium: Option<PathBuf>,
    /// Chromium's runtime files for the desktop window; else `lib/cef` in the
    /// runtime pack.
    cef_dir: Option<PathBuf>,
    /// Extra Chromium switches for the window's UI page, `name[=value]`.
    cef_switches: Vec<String>,
    /// Where the host writes its NDJSON diagnostics.
    diagnostics_log: Option<PathBuf>,
    /// The file whose time marks the host's last use (`rusty dev` idle expiry).
    activity_file: Option<PathBuf>,
    /// The build configuration a `--performance-probe` result names.
    performance_configuration: Option<String>,
    runtime_instance_id: Option<RuntimeInstanceId>,
    /// Set on the runtime process a supervisor starts: serve this inherited
    /// listener instead of binding one.
    serve_listener_fd: Option<i32>,
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

    const fn identifier(self) -> &'static str {
        match self {
            Self::NativeAot => "nativeaot",
            Self::CoreClr => "coreclr",
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
    /// Packaged CoreCLR launches, and any `--supervised` or `--headless`
    /// launch, run the runtime as a child of a signal-owning supervisor with
    /// the selected loader; finite probes stay in process.
    fn uses_supervisor(&self) -> bool {
        self.serve_listener_fd.is_none()
            && (self.supervised
                || self.headless
                || (matches!(self.loader, ProductLoader::CoreClr)
                    && self.product_path.is_some()
                    && !self.exercise
                    && self.performance_probe.is_none()))
    }

    /// The launch options a supervisor hands the runtime process it starts.
    #[cfg(unix)]
    fn runtime_forwarded_arguments(&self) -> Result<Vec<String>, String> {
        let path = |path: &Path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("`{}` is not UTF-8", path.display()))
        };
        let mut arguments = Vec::new();
        if let Some(directory) = &self.cef_dir {
            arguments.extend(["--cef-dir".to_owned(), path(directory)?]);
        }
        for switch in &self.cef_switches {
            arguments.extend(["--cef-switch".to_owned(), switch.clone()]);
        }
        if let Some(file) = &self.diagnostics_log {
            arguments.extend(["--diagnostics-log".to_owned(), path(file)?]);
        }
        if let Some(file) = &self.activity_file {
            arguments.extend(["--activity-file".to_owned(), path(file)?]);
        }
        Ok(arguments)
    }

    /// Where diagnostics are written: `--diagnostics-log`, else the default.
    fn log_config(&self) -> ProductHostLogConfig {
        let config = ProductHostLogConfig::default();
        match &self.diagnostics_log {
            Some(path) => config.with_path(path),
            None => config,
        }
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
        if let Some(product) = &self.product {
            let (world_lights, viewmodel_lights) = product.default_lights();
            config = config
                .with_input_cursor_mode(product.input_cursor_mode.native())
                .with_default_lights(world_lights, viewmodel_lights)
                .with_scene_shadows(product.shadows_enabled())
                .with_shadow_budget(product.shadow_budget())
                .with_audio_output(product.audio_output)
                .with_product(&product.id, &product.title);
        }
        if let Some(root) = &self.persistence_root {
            config = config.with_persistence_root(root.clone());
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

    fn content(
        &self,
    ) -> Result<CsharpProductContent, csharp_product_runtime::CsharpProductRuntimeError> {
        match &self.product {
            Some(product) => {
                CsharpProductContent::admit_from(&product.source, &product.content_root)
            }
            None => CsharpProductContent::admit(
                self.content_dir
                    .as_deref()
                    .expect("legacy content required"),
            ),
        }
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
        let mut live_debug = false;
        let mut exercise = false;
        let mut performance_probe = None;
        let mut supervised = false;
        let mut debugger = false;
        let mut headless = false;
        let mut chromium = None;
        let mut cef_dir = None;
        let mut cef_switches = Vec::new();
        let mut diagnostics_log = None;
        let mut activity_file = None;
        let mut performance_configuration = None;
        let mut runtime_instance_id = None;
        let mut serve_listener_fd = None;
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
                "--headless" => headless = true,
                "--chromium" => {
                    chromium = Some(PathBuf::from(
                        values.next().ok_or("--chromium requires an executable")?,
                    ))
                }
                "--cef-dir" => {
                    cef_dir = Some(PathBuf::from(
                        values.next().ok_or("--cef-dir requires a directory")?,
                    ))
                }
                "--cef-switch" => {
                    cef_switches.push(values.next().ok_or("--cef-switch requires name[=value]")?)
                }
                "--diagnostics-log" => {
                    diagnostics_log = Some(PathBuf::from(
                        values.next().ok_or("--diagnostics-log requires a file")?,
                    ))
                }
                "--activity-file" => {
                    activity_file = Some(PathBuf::from(
                        values.next().ok_or("--activity-file requires a file")?,
                    ))
                }
                "--performance-configuration" => {
                    performance_configuration = Some(
                        values
                            .next()
                            .ok_or("--performance-configuration requires a label")?,
                    )
                }
                "--serve-listener-fd" => {
                    serve_listener_fd = Some(
                        values
                            .next()
                            .ok_or("--serve-listener-fd requires a file descriptor")?
                            .parse::<i32>()
                            .map_err(|_| "--serve-listener-fd must be a file descriptor")?,
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
                        "usage: rusty-product-host --product <Product-directory|product.rpak> --loader <nativeaot|coreclr> [--supervised] [--debugger] [--headless [--chromium <executable>]] [--cef-dir <directory>] [--cef-switch <name[=value]>]... [--diagnostics-log <file>] [--activity-file <file>] [--runtime-instance-id <nonzero-u64>] [--persistence-root <absolute-path>] [--exercise] [--performance-probe <1..=256> [--performance-configuration <label>]]\n\nThe Product directory contains product.json plus its declared managed/native artifacts, UI, and admitted content. A release Product is one container file (`rusty build --pack`) holding product.json, UI and content, with the managed/native artifacts loose beside it. The matched Engine browser shell is discovered beside this runtime-pack binary; Product directories never carry Engine JavaScript. `--loader` chooses one exact optional manifest artifact. `--exercise` runs Engine provider-fixture assertions (voxel/UI/input/timeline/fault behavior), not a general product health check; ordinary products should omit it. See docs/csharp-product-project.md#host-exercise-contract. `--supervised` is the explicit rusty-dev stdin-close shutdown hook. `--debugger` disables the CoreCLR runtime startup deadline for managed debugging; shutdown remains bounded. `--headless` opens the page in headless Chromium after the listener is ready, so an unattended run keeps drawing and mounts the product UI (animation and video completions flow without it), and closes it with the host; `--chromium` selects its executable, else one on `PATH`. The product manifest's `renderer.output` selects stream or window output; in window output `--cef-dir` overrides the runtime pack's `lib/cef` and each `--cef-switch` adds a Chromium switch for the UI page (for example `remote-debugging-port=9333`). `--diagnostics-log` writes the host's NDJSON diagnostics to that file. `--activity-file` marks the host's last use (input, control, live debug, a page attaching; not frame pulls) in that file's time, which `rusty dev` reads to stop idle sessions. `--runtime-instance-id` names this host-owned runtime incarnation; direct launches allocate a process-local fallback when it is omitted. Server bind/port and explicit liveDebug opt-in are Product metadata. `--identity` prints machine-readable matched runtime identity; `--version` prints a concise diagnostic identity.\n\n{PHYSICAL_MAPPING_USAGE}"
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
        let product = product
            .map(|path| {
                let source = csharp_product_runtime::ProductSource::open(&path)
                    .map_err(|error| format!("--product: {error}"))?;
                ProductBundle::read(&source)
            })
            .transpose()?;
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
            legacy_live_debug: live_debug,
            exercise,
            performance_probe,
            supervised,
            debugger,
            headless,
            chromium,
            cef_dir,
            cef_switches,
            diagnostics_log,
            activity_file,
            performance_configuration,
            runtime_instance_id,
            serve_listener_fd,
        };
        if arguments.debugger
            && (!matches!(arguments.loader, ProductLoader::CoreClr) || !arguments.uses_supervisor())
        {
            return Err("--debugger requires a supervised CoreCLR product host".to_owned());
        }
        if arguments.headless && arguments.serve_listener_fd.is_some() {
            return Err("--headless is only valid for a foreground product host".to_owned());
        }
        if arguments.headless && (arguments.exercise || arguments.performance_probe.is_some()) {
            return Err(
                "--headless cannot be combined with --exercise or --performance-probe".to_owned(),
            );
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
        "controller-button-value" => RuntimeInputTrigger::ControllerButtonValue {
            button: parse_controller_button(next_mapping_token(&mut tokens, "controller button")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "controller-axis" => RuntimeInputTrigger::ControllerAxis {
            axis: parse_controller_axis(next_mapping_token(&mut tokens, "controller axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        _ => {
            return Err(format!(
                "--physical-mapping trigger `{trigger_kind}` is unsupported; expected key, pointer-button, pointer-axis, wheel, controller-button, controller-button-value, or controller-axis"
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
        "arrow-up" => KeyboardControl::ArrowUp,
        "arrow-down" => KeyboardControl::ArrowDown,
        "arrow-left" => KeyboardControl::ArrowLeft,
        "arrow-right" => KeyboardControl::ArrowRight,
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

fn load_bundle(root: &Path, product: &ProductBundle) -> Result<ProductHostBundle, String> {
    let mut entries = Vec::new();
    collect_bundle(root, root, &mut entries)?;
    entries.extend(product.browser_entries()?);
    ProductHostBundle::new(entries).map_err(|error| error.to_string())
}

fn load_legacy_bundle(root: &Path) -> Result<ProductHostBundle, String> {
    let mut entries = Vec::new();
    collect_bundle(root, root, &mut entries)?;
    ProductHostBundle::new(entries).map_err(|error| error.to_string())
}

fn collect_bundle(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ProductHostBundleEntry>,
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
                ProductHostBundleEntry::new(
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
        "wav" => Some("audio/wav"),
        "ogg" | "opus" => Some("audio/ogg"),
        "mp3" => Some("audio/mpeg"),
        "flac" => Some("audio/flac"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn debugger_is_coreclr_supervisor_scoped() {
        assert!(parse_test_error(&["--debugger", "--supervised"])
            .contains("requires a supervised CoreCLR product host"));
        assert!(parse_test_error(&[
            "--debugger",
            "--loader",
            "coreclr",
            "--runtimeconfig",
            "product.runtimeconfig.json",
        ])
        .contains("requires a supervised CoreCLR product host"));
        let debugging = parse_test_args(&[
            "--debugger",
            "--supervised",
            "--loader",
            "coreclr",
            "--runtimeconfig",
            "product.runtimeconfig.json",
        ])
        .expect("explicit debugger mode");
        assert!(debugging.debugger && debugging.uses_supervisor());
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
            "controller-button-value=axis",
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
            "controller-button-value=controller-button-value:controller-button-value:button-7",
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
            RuntimeInputTrigger::ControllerButtonValue { .. }
        ));
        assert!(matches!(
            mappings[6].trigger(),
            RuntimeInputTrigger::ControllerAxis { .. }
        ));
    }

    #[test]
    fn parser_admits_arrow_mappings_and_matches_wire_names() {
        for (name, expected) in [
            ("arrow-up", KeyboardControl::ArrowUp),
            ("arrow-down", KeyboardControl::ArrowDown),
            ("arrow-left", KeyboardControl::ArrowLeft),
            ("arrow-right", KeyboardControl::ArrowRight),
            ("enter", KeyboardControl::Enter),
        ] {
            let mapping =
                parse_physical_mapping(&format!("navigate=menu.navigate:key:{name}:pressed"))
                    .unwrap();
            let RuntimeInputTrigger::Key { code, .. } = mapping.trigger() else {
                panic!("key mapping");
            };
            assert_eq!(*code, expected);
            assert_eq!(serde_json::to_value(expected).unwrap(), name);
            assert_eq!(
                serde_json::from_value::<KeyboardControl>(serde_json::json!(name)).unwrap(),
                expected
            );
        }
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
    fn parser_admits_headless_only_for_foreground_host_launches() {
        let args = parse_test_args(&["--headless"]).expect("headless host launch parses");
        assert!(args.headless);
        assert!(parse_test_error(&["--headless", "--exercise"])
            .contains("cannot be combined with --exercise"));
        assert!(
            parse_test_error(&["--headless", "--serve-listener-fd", "3"])
                .contains("only valid for a foreground product host")
        );
    }

    #[test]
    fn headless_requires_a_packaged_product() {
        let mut args = parse_test_args(&["--headless"]).expect("headless host arguments");
        assert!(validate_headless_host(&args)
            .expect_err("legacy direct launches cannot use the supervisor")
            .contains("requires a packaged --product"));

        args.product_path = Some(PathBuf::from("/product"));
        validate_headless_host(&args).expect("packaged headless host is supported");
    }

    #[test]
    fn packaged_coreclr_uses_the_supervisor_but_finite_probes_do_not() {
        let mut args = parse_test_args(&[
            "--loader",
            "coreclr",
            "--runtimeconfig",
            "product.runtimeconfig.json",
        ])
        .unwrap();
        assert!(
            !args.uses_supervisor(),
            "legacy raw artifacts stay in process"
        );
        args.product_path = Some(PathBuf::from("/product"));
        assert!(args.uses_supervisor());
        assert!(
            !args.supervised,
            "direct hosts must not adopt stdin supervision"
        );
        args.serve_listener_fd = Some(3);
        assert!(
            !args.uses_supervisor(),
            "a supervised runtime must not launch another supervisor"
        );
        args.serve_listener_fd = None;
        args.exercise = true;
        assert!(!args.uses_supervisor());
        args.exercise = false;
        args.performance_probe = Some(1);
        assert!(!args.uses_supervisor());
        assert!(
            !parse_test_args(&[]).unwrap().uses_supervisor(),
            "NativeAOT stays in process"
        );
    }

    #[cfg(unix)]
    #[test]
    fn headless_browser_uses_loopback_for_a_wildcard_listener() {
        assert_eq!(
            browser_url("0.0.0.0:9348".parse().unwrap()),
            "http://127.0.0.1:9348"
        );
        assert_eq!(
            browser_url("127.0.0.2:9348".parse().unwrap()),
            "http://127.0.0.2:9348"
        );
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
