use std::{
    env, fs,
    io::{Read, Write},
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
use product_dev_host::{
    ProductDevBundle, ProductDevBundleEntry, ProductDevHost, ProductDevHostConfig, ProductDevLog,
    ProductDevRendererResource,
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
        ProductDevHostConfig::new(args.port(), bundle)
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
        wait_for_process_termination(args.supervised);
    }
    Ok(())
}

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
fn wait_for_process_termination(supervised: bool) {
    let termination = install_termination_signal_hook();
    if supervised {
        let mut input = std::io::stdin();
        let mut byte = [0_u8; 1];
        while !termination.load(Ordering::Relaxed)
            && input.read(&mut byte).is_ok_and(|count| count != 0)
        {}
        let reason = if termination.load(Ordering::Relaxed) {
            "termination-signal"
        } else {
            "supervisor-stdin-closed"
        };
        println!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}");
    } else {
        while !termination.load(Ordering::Relaxed) {
            std::thread::park_timeout(std::time::Duration::from_millis(100));
        }
        println!("RUSTY_HOST shutdown={{\"reason\":\"termination-signal\"}}");
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
    runtime_instance_id: Option<RuntimeInstanceId>,
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
        let mut runtime_instance_id = None;
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
                        return Err(format!("--physical-mapping accepts at most {MAX_PHYSICAL_MAPPINGS} declarations"));
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
                        "usage: rusty-product-host --product <Product-directory> --loader <nativeaot|coreclr> [--supervised] [--runtime-instance-id <nonzero-u64>] [--persistence-root <absolute-path>] [--content-store-root <absolute-path>] [--exercise] [--performance-probe <1..=256>]\n\nThe Product directory contains product.json plus its declared managed/native artifacts, UI, and admitted content. The matched Engine browser shell is discovered beside this runtime-pack binary; Product directories never carry Engine JavaScript. `--loader` chooses one exact optional manifest artifact. `--supervised` is the explicit rusty-dev stdin-close shutdown hook. `--runtime-instance-id` names this host-owned runtime incarnation; direct launches allocate a process-local fallback when it is omitted. Server bind/port and explicit liveDebug opt-in are Product metadata. `--identity` prints machine-readable matched runtime identity; `--version` prints a concise diagnostic identity.\n\n{PHYSICAL_MAPPING_USAGE}"
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
                )
            }
            (None, Some(loader)) => loader,
            (None, None) => ProductLoader::NativeAot,
        };
        let arguments = Self {
            loader,
            product,
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
            runtime_instance_id,
        };
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
                    )
                }
                (ProductLoader::NativeAot, Some(_)) => {
                    return Err("--runtimeconfig is only valid with --loader coreclr".to_owned())
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
        _ => {
            return Err(format!(
                "--physical-mapping controller button `{value}` is unsupported"
            ));
        }
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
