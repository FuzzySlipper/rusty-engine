//! The `rusty` command: the downstream product workflow for Rusty Engine.
//!
//! It installs and updates the product's pinned SDK/runtime pair, reports what
//! is selected and missing, and builds and runs the product. It holds no
//! Product configuration: the SDK evaluates and stages that truth, and the
//! pinned runtime pack supplies the exact host that `rusty dev` starts.

mod pair;

use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitCode, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use serde::Deserialize;
use serde_json::Value;

use pair::{InstalledPair, Pin};

const STAGE_TARGET: &str = "StageRustyEngineCoreClrProduct";
const AOT_TARGET: &str = "VerifyRustyEngineAot";
const STAGE_ASSETS_TARGET: &str = "StageRustyEngineProductAssets";
const STAGED_PRODUCT_PROPERTY: &str = "RustyEngineStagedProductDirectory";
const WATCH_PATHS_PROPERTY: &str = "RustyEngineWatchPaths";
const UI_SOURCE_ROOT_PROPERTY: &str = "RustyEngineProductUiSourceRoot";
const UI_ROOT_PROPERTY: &str = "RustyEngineProductUiRoot";
const CONTENT_ROOT_PROPERTY: &str = "RustyEngineProductContentRoot";
const CONTENT_BUNDLE_ITEM: &str = "RustyEngineContentBundle";
const REQUIRED_DOTNET_MAJOR: u32 = 10;
const BOOTSTRAP_URL: &str =
    "https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh";
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const UNEXPECTED_EXIT_RESTART_BACKOFF: Duration = Duration::from_millis(100);
const MAX_UNEXPECTED_EXITS_PER_ARTIFACT: u8 = 2;
const MAX_SUPERVISOR_COMMAND_BYTES: usize = 16 * 1024;
static NEXT_SUPERVISED_RUNTIME_INSTANCE_ID: AtomicU64 = AtomicU64::new(0);
const IGNORED_WATCH_DIRECTORY_NAMES: &[&str] = &[
    ".git",
    ".idea",
    ".runtime",
    ".vs",
    ".vscode",
    "bin",
    "dist",
    "generated",
    "node_modules",
    "obj",
    "target",
];

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    match Arguments::parse(env::args().skip(1))?.command {
        CommandName::Help(text) => {
            println!("{text}");
            Ok(ExitCode::SUCCESS)
        }
        CommandName::Dev(options) => dev(options).map(|()| ExitCode::SUCCESS),
        CommandName::Build(options) => build(&options),
        CommandName::Install(options) => install(&options),
        CommandName::Update(options) => update(&options),
        CommandName::Status(options) => status(&options),
    }
}

#[cfg(unix)]
fn install_termination_signal_hook() -> Result<Arc<AtomicBool>, String> {
    let requested = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&requested))
            .map_err(|error| format!("RUSTY_DEV_SIGNAL: {error}"))?;
    }
    Ok(requested)
}

#[cfg(not(unix))]
fn install_termination_signal_hook() -> Result<Arc<AtomicBool>, String> {
    Ok(Arc::new(AtomicBool::new(false)))
}

fn dev(mut options: DevOptions) -> Result<(), String> {
    use_dotnet_root_for_host();
    if options.engine_source.is_none() {
        let pinned = if options.runtime.is_none() {
            pinned_pair(&options.project)?
        } else {
            pinned_pair_if_installed(&options.project)?
        };
        if let Some((pin, pair)) = pinned {
            warn_shape(&pin, &absolute(&options.project)?);
            if options.runtime.is_none() {
                // Window mode runs the pair's desktop pack: the same host with
                // the desktop shell and Chromium's runtime.
                let runtime = if window_output() {
                    pair::install_desktop_pack(&pair)?.0
                } else {
                    pair.runtime_pack()
                };
                diagnostic(
                    "pin-resolved",
                    serde_json::json!({
                        "pin": pin.version,
                        "pinFile": pin.file,
                        "runtimePack": runtime,
                    }),
                );
                delegate_to_pair_cli(&runtime, &options)?;
                options.runtime = Some(runtime);
            }
        }
    }
    let termination = install_termination_signal_hook()?;
    let runtime = RuntimePack::resolve(&options)?;
    runtime.verify()?;

    let persistence_root = development_persistence_root(&options.project)?;
    let initial = stage_product(&options)?;
    let mut staged = initial.directory;
    verify_staged_product(&staged)?;
    let mut watches = initial.watches;
    let mut asset_roots = initial.asset_roots;
    let mut snapshot = FileSnapshot::capture(&watches)?;
    let mut child = Some(SupervisedHost::start(
        &runtime.host,
        &staged,
        &persistence_root,
        options.debugger,
        options.headless,
    )?);
    let mut crash_budget = CrashBudget::new(MAX_UNEXPECTED_EXITS_PER_ARTIFACT);

    diagnostic(
        "started",
        serde_json::json!({
            "project": options.project,
            "productDirectory": staged,
            "persistenceRoot": persistence_root,
            "runtimePack": runtime.root,
            "loader": "coreclr",
            "watchPaths": watches,
            "pid": child.as_ref().expect("initial child is present").child.id(),
            "runtimeInstanceId": child.as_ref().expect("initial child is present").runtime_instance_id,
        }),
    );

    loop {
        if termination.load(Ordering::Acquire) {
            if let Some(mut active_child) = child.take() {
                active_child.shutdown()?;
            }
            diagnostic(
                "stopped",
                serde_json::json!({ "reason": "termination-signal" }),
            );
            return Ok(());
        }
        if let Some(active_child) = child.as_mut() {
            if let Some(status) = active_child.try_wait()? {
                let exited_child = child.take().expect("observed child is present");
                diagnostic(
                    "child-exited-unexpectedly",
                    serde_json::json!({
                        "pid": exited_child.child.id(),
                        "status": status.code(),
                        "runtimeInstanceId": exited_child.runtime_instance_id,
                    }),
                );
                match crash_budget.record_unexpected_exit() {
                    CrashBudgetDecision::Restart { backoff } => {
                        diagnostic(
                            "restart-backoff",
                            serde_json::json!({
                                "backoffMs": backoff.as_millis(),
                                "unexpectedExitCount": crash_budget.unexpected_exit_count(),
                            }),
                        );
                        thread::sleep(backoff);
                        let restarted = SupervisedHost::start(
                            &runtime.host,
                            &staged,
                            &persistence_root,
                            options.debugger,
                            options.headless,
                        )?;
                        diagnostic(
                            "restarted-after-unexpected-exit",
                            serde_json::json!({
                                "pid": restarted.child.id(),
                                "runtimeInstanceId": restarted.runtime_instance_id,
                                "unexpectedExitCount": crash_budget.unexpected_exit_count(),
                            }),
                        );
                        child = Some(restarted);
                    }
                    CrashBudgetDecision::PausedFault => {
                        diagnostic(
                            "paused-fault",
                            serde_json::json!({
                                "reason": "unexpected-child-exit-budget-exhausted",
                                "unexpectedExitCount": crash_budget.unexpected_exit_count(),
                                "crashBudget": crash_budget.limit(),
                                "productDirectory": staged,
                            }),
                        );
                    }
                }
            }
        }
        thread::sleep(POLL_INTERVAL);
        let next = match FileSnapshot::capture(&watches) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                diagnostic(
                    "watch-snapshot-failed",
                    serde_json::json!({
                        "watchPaths": watches,
                        "error": error,
                    }),
                );
                continue;
            }
        };
        let changed = snapshot.changed_paths(&next);
        if !record_observed_snapshot(&mut snapshot, next) {
            continue;
        }

        let assets_only = changed
            .iter()
            .all(|path| asset_roots.iter().any(|root| path.starts_with(root)));
        diagnostic(
            "change-detected",
            serde_json::json!({ "watchPaths": watches, "changed": changed, "assetsOnly": assets_only }),
        );
        // UI and bundle content edits restage only those files and reload
        // them into the running product: no C# build, no replacement.
        if assets_only {
            if let Some(active_child) = child.as_mut() {
                let reloaded = stage_assets(&options).and_then(|()| active_child.reload_assets());
                match reloaded {
                    // The runtime reports its own reload result.
                    Ok(()) => diagnostic(
                        "assets-restaged",
                        serde_json::json!({ "productDirectory": staged, "changed": changed }),
                    ),
                    Err(error) => diagnostic(
                        "restage-failed",
                        serde_json::json!({ "phase": "stage-assets", "error": error }),
                    ),
                }
                continue;
            }
        }
        let StagedProduct {
            directory: next_staged,
            watches: refreshed_watches,
            asset_roots: refreshed_asset_roots,
        } = match stage_product(&options) {
            Ok(staged) => staged,
            Err(error) => {
                diagnostic(
                    "restage-failed",
                    serde_json::json!({
                        "phase": "stage-product",
                        "error": error,
                    }),
                );
                continue;
            }
        };
        if let Err(error) = verify_staged_product(&next_staged) {
            diagnostic(
                "restage-failed",
                serde_json::json!({
                    "phase": "verify-staged-product",
                    "productDirectory": next_staged,
                    "error": error,
                }),
            );
            continue;
        }
        let refreshed_snapshot = match FileSnapshot::capture(&refreshed_watches) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                diagnostic(
                    "restage-failed",
                    serde_json::json!({
                        "phase": "capture-refreshed-watch-snapshot",
                        "watchPaths": refreshed_watches,
                        "error": error,
                    }),
                );
                continue;
            }
        };
        snapshot = refreshed_snapshot;
        watches = refreshed_watches;
        asset_roots = refreshed_asset_roots;
        let mut replacement_failed = false;
        let started_after_restage = if let Some(active_child) = child.as_mut() {
            if let Some(status) = active_child.try_wait()? {
                diagnostic(
                    "child-exited-during-restage",
                    serde_json::json!({
                        "pid": active_child.child.id(),
                        "status": status.code(),
                        "runtimeInstanceId": active_child.runtime_instance_id,
                    }),
                );
                child.take();
                true
            } else {
                match active_child.replace_runtime(&next_staged) {
                    Ok(()) => false,
                    Err(error) => {
                        if let Some(status) = active_child.try_wait()? {
                            diagnostic(
                                "child-exited-during-restage",
                                serde_json::json!({
                                    "pid": active_child.child.id(),
                                    "status": status.code(),
                                    "runtimeInstanceId": active_child.runtime_instance_id,
                                }),
                            );
                            child.take();
                            true
                        } else {
                            diagnostic(
                                "restage-failed",
                                serde_json::json!({
                                    "phase": "replace-runtime",
                                    "productDirectory": next_staged,
                                    "error": error,
                                }),
                            );
                            replacement_failed = true;
                            false
                        }
                    }
                }
            }
        } else {
            child = Some(SupervisedHost::start(
                &runtime.host,
                &next_staged,
                &persistence_root,
                options.debugger,
                options.headless,
            )?);
            true
        };
        if started_after_restage && child.is_none() {
            child = Some(SupervisedHost::start(
                &runtime.host,
                &next_staged,
                &persistence_root,
                options.debugger,
                options.headless,
            )?);
        }
        if replacement_failed {
            continue;
        }
        crash_budget.reset_after_successful_restage();
        if started_after_restage {
            diagnostic(
                "started-after-restage",
                serde_json::json!({
                    "productDirectory": next_staged,
                    "persistenceRoot": persistence_root,
                    "loader": "coreclr",
                    "watchPaths": watches,
                    "pid": child.as_ref().expect("restaged child is present").child.id(),
                    "shellRuntimeSeedInstanceId": child.as_ref().expect("restaged child is present").runtime_instance_id,
                    "crashBudgetReset": true,
                }),
            );
        } else {
            diagnostic(
                "runtime-replaced",
                serde_json::json!({
                    "productDirectory": next_staged,
                    "persistenceRoot": persistence_root,
                    "loader": "coreclr",
                    "watchPaths": watches,
                    "pid": child.as_ref().expect("restaged child is present").child.id(),
                    "crashBudgetReset": true,
                }),
            );
        }
        staged = next_staged;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrashBudgetDecision {
    Restart { backoff: Duration },
    PausedFault,
}

/// Pure, per-staged-artifact crash-loop state. It deliberately does not infer
/// health from process age or an HTTP request: only a successful source
/// restage permits another replacement attempt after the budget is exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CrashBudget {
    limit: u8,
    unexpected_exits: u8,
}

impl CrashBudget {
    const fn new(limit: u8) -> Self {
        Self {
            limit,
            unexpected_exits: 0,
        }
    }

    fn record_unexpected_exit(&mut self) -> CrashBudgetDecision {
        self.unexpected_exits = self.unexpected_exits.saturating_add(1);
        if self.unexpected_exits >= self.limit {
            CrashBudgetDecision::PausedFault
        } else {
            CrashBudgetDecision::Restart {
                backoff: UNEXPECTED_EXIT_RESTART_BACKOFF,
            }
        }
    }

    const fn reset_after_successful_restage(&mut self) {
        self.unexpected_exits = 0;
    }

    const fn unexpected_exit_count(self) -> u8 {
        self.unexpected_exits
    }

    const fn limit(self) -> u8 {
        self.limit
    }
}

#[derive(Debug)]
struct Arguments {
    command: CommandName,
}

#[derive(Debug)]
enum CommandName {
    Help(String),
    Dev(DevOptions),
    Build(BuildOptions),
    Install(InstallOptions),
    Update(UpdateOptions),
    Status(StatusOptions),
}

#[derive(Debug)]
struct DevOptions {
    project: PathBuf,
    runtime: Option<PathBuf>,
    engine_source: Option<PathBuf>,
    bind_host: Option<String>,
    port: Option<u16>,
    live_debug: bool,
    debugger: bool,
    headless: bool,
}

#[derive(Debug)]
struct BuildOptions {
    project: PathBuf,
    engine_source: Option<PathBuf>,
    aot: bool,
}

#[derive(Debug)]
struct InstallOptions {
    project: Option<PathBuf>,
    archive: Option<PathBuf>,
}

#[derive(Debug)]
struct UpdateOptions {
    project: Option<PathBuf>,
    to: Option<String>,
    check: bool,
}

#[derive(Debug)]
struct StatusOptions {
    project: Option<PathBuf>,
}

impl Arguments {
    fn parse(values: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut values = values.into_iter();
        let command = match values.next() {
            None => return Ok(Self::help(usage())),
            Some(command) => command,
        };
        let rest: Vec<String> = values.collect();
        let help = |text: fn() -> String| {
            rest.iter()
                .any(|value| value == "--help" || value == "-h")
                .then(|| Self::help(text()))
        };
        let command = match command.as_str() {
            "help" | "--help" | "-h" => return Ok(Self::help(usage())),
            "dev" => match help(dev_usage) {
                Some(help) => return Ok(help),
                None => CommandName::Dev(parse_dev(rest)?),
            },
            "build" => match help(build_usage) {
                Some(help) => return Ok(help),
                None => CommandName::Build(parse_build(rest)?),
            },
            "install" => match help(install_usage) {
                Some(help) => return Ok(help),
                None => CommandName::Install(parse_install(rest)?),
            },
            "update" => match help(update_usage) {
                Some(help) => return Ok(help),
                None => CommandName::Update(parse_update(rest)?),
            },
            "status" => match help(status_usage) {
                Some(help) => return Ok(help),
                None => CommandName::Status(parse_status(rest, "status", status_usage)?),
            },
            other => {
                return Err(format!(
                    "RUSTY_ARGUMENT: unknown command `{other}`\n\n{}",
                    usage()
                ))
            }
        };
        Ok(Self { command })
    }

    const fn help(text: String) -> Self {
        Self {
            command: CommandName::Help(text),
        }
    }
}

fn parse_dev(values: Vec<String>) -> Result<DevOptions, String> {
    let mut values = values.into_iter();
    let mut project = None;
    let mut runtime = None;
    let mut engine_source = None;
    let mut bind_host = None;
    let mut port = None;
    let mut live_debug = false;
    let mut debugger = false;
    let mut headless = false;
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => project = Some(PathBuf::from(required_value(&mut values, "--project")?)),
            "--runtime" => runtime = Some(PathBuf::from(required_value(&mut values, "--runtime")?)),
            "--engine-source" => {
                engine_source = Some(PathBuf::from(required_value(
                    &mut values,
                    "--engine-source",
                )?))
            }
            "--bind-host" => {
                let value = required_value(&mut values, "--bind-host")?;
                value
                    .parse::<std::net::Ipv4Addr>()
                    .map_err(|_| "RUSTY_DEV_ARGUMENT: --bind-host must be an IPv4 address")?;
                bind_host = Some(value);
            }
            "--port" => {
                port = Some(
                    required_value(&mut values, "--port")?
                        .parse()
                        .map_err(|_| "RUSTY_DEV_ARGUMENT: --port must be a u16")?,
                )
            }
            "--live-debug" => live_debug = true,
            "--debugger" => debugger = true,
            "--headless" => headless = true,
            _ => return Err(unknown_argument("dev", &value, dev_usage)),
        }
    }
    if runtime.is_some() && engine_source.is_some() {
        return Err(
            "RUSTY_DEV_ARGUMENT: --runtime and --engine-source are mutually exclusive".to_owned(),
        );
    }
    let project = project.ok_or_else(|| {
        "RUSTY_DEV_ARGUMENT: --project <ordinary-product.csproj> is required".to_owned()
    })?;
    Ok(DevOptions {
        project,
        runtime,
        engine_source,
        bind_host,
        port,
        live_debug,
        debugger,
        headless,
    })
}

fn parse_build(values: Vec<String>) -> Result<BuildOptions, String> {
    let mut values = values.into_iter();
    let mut project = None;
    let mut engine_source = None;
    let mut aot = false;
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => project = Some(PathBuf::from(required_value(&mut values, "--project")?)),
            "--engine-source" => {
                engine_source = Some(PathBuf::from(required_value(
                    &mut values,
                    "--engine-source",
                )?))
            }
            "--aot" => aot = true,
            _ => return Err(unknown_argument("build", &value, build_usage)),
        }
    }
    Ok(BuildOptions {
        project: project.ok_or("RUSTY_ARGUMENT: rusty build needs --project <product.csproj>")?,
        engine_source,
        aot,
    })
}

fn parse_install(values: Vec<String>) -> Result<InstallOptions, String> {
    let mut values = values.into_iter();
    let mut options = InstallOptions {
        project: None,
        archive: None,
    };
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => {
                options.project = Some(PathBuf::from(required_value(&mut values, "--project")?))
            }
            "--archive" => {
                options.archive = Some(PathBuf::from(required_value(&mut values, "--archive")?))
            }
            _ => return Err(unknown_argument("install", &value, install_usage)),
        }
    }
    Ok(options)
}

fn parse_update(values: Vec<String>) -> Result<UpdateOptions, String> {
    let mut values = values.into_iter();
    let mut options = UpdateOptions {
        project: None,
        to: None,
        check: false,
    };
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => {
                options.project = Some(PathBuf::from(required_value(&mut values, "--project")?))
            }
            "--to" => {
                let version = required_value(&mut values, "--to")?;
                pair::validate_version(&version)?;
                options.to = Some(version);
            }
            "--check" => options.check = true,
            _ => return Err(unknown_argument("update", &value, update_usage)),
        }
    }
    Ok(options)
}

fn parse_status(
    values: Vec<String>,
    command: &str,
    usage: fn() -> String,
) -> Result<StatusOptions, String> {
    let mut values = values.into_iter();
    let mut options = StatusOptions { project: None };
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => {
                options.project = Some(PathBuf::from(required_value(&mut values, "--project")?))
            }
            _ => return Err(unknown_argument(command, &value, usage)),
        }
    }
    Ok(options)
}

fn required_value(values: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    values
        .next()
        .ok_or_else(|| format!("RUSTY_ARGUMENT: {flag} requires a value"))
}

fn unknown_argument(command: &str, value: &str, usage: fn() -> String) -> String {
    format!(
        "RUSTY_ARGUMENT: rusty {command} does not accept `{value}`\n\n{}",
        usage()
    )
}

fn usage() -> String {
    format!(
        "rusty: build, run and update a Rusty Engine product

usage: rusty <command> [options]

commands:
  status    show the pinned Engine pair, whether it is installed, paths, and missing prerequisites
  install   install the pinned SDK/runtime pair into the shared cache (once; later use works offline)
  update    move the pin to a newer published pair, install it, and list what changed
  build     restore, build and stage the product; --aot also publishes NativeAOT
  dev       build and run the product on its pinned runtime, rebuilding on source changes

Run `rusty <command> --help` for a command's options.

Everyday use, from the product repository:
  rusty status
  rusty install
  rusty dev --project src/Game/Game.csproj --port 8787
  rusty update --check
  rusty update
  rusty build --project src/Game/Game.csproj --aot

The pin is the one <{pin}> element in the product's {pin_file}.
Nothing moves it except `rusty update`. Installed pairs live in {cache}
(set {cache_variable} to move it).

Get or refresh this command:
  curl -fsSL {bootstrap} | bash",
        pin = pair::PIN_ELEMENT,
        pin_file = pair::PIN_FILE,
        cache = pair::cache_root().map_or_else(|error| error, |root| root.display().to_string()),
        cache_variable = pair::CACHE_VARIABLE,
        bootstrap = BOOTSTRAP_URL,
    )
}

fn dev_usage() -> String {
    "usage: rusty dev --project <ordinary-product.csproj> [--port <u16>] [--bind-host <IPv4>] [--live-debug] [--debugger] [--headless]
                 [--runtime <runtime-pack> | --engine-source <rusty-engine-source>]

Builds and stages the product through its SDK, starts it on CoreCLR, and restages when declared
C#, UI or content inputs change. UI and content-bundle edits reload into the running product; other
edits replace the runtime.

The runtime is the pair pinned in the product's Directory.Build.props, installed by `rusty install`.
`rusty dev` runs that pair's own copy of this command, so the supervisor always matches its host.
With RUSTY_RENDER_OUTPUT=window the product opens in a native window; the first such run downloads
the pair's desktop runtime pack (Chromium's runtime for the UI) into the cache beside the pair.

  --port, --bind-host  where the browser host listens
  --live-debug         enable the live-debug command surface
  --debugger           no runtime startup deadline, for managed breakpoints
  --headless           run unattended: a headless Chromium page keeps the world drawing and the UI mounted (RUSTY_CHROMIUM_PATH selects it)
  --runtime            Engine contributors: use this runtime pack instead of the pin
  --engine-source      Engine contributors: build the SDK and runtime from this checkout

This command never invokes Cargo and never searches for an adjacent Engine checkout.

Examples:
  rusty dev --project src/Game/Game.csproj --port 8787
  rusty dev --project src/Game/Game.csproj --live-debug --headless
  RUSTY_RENDER_OUTPUT=window rusty dev --project src/Game/Game.csproj"
        .to_owned()
}

fn build_usage() -> String {
    "usage: rusty build --project <product.csproj> [--aot] [--engine-source <rusty-engine-source>]

Restores against the pinned SDK in the shared cache, builds, and stages the CoreCLR product bundle
(the SDK target StageRustyEngineCoreClrProduct). --aot runs VerifyRustyEngineAot, which also
publishes the NativeAOT product. Compiler output and dotnet's exit code are passed through.

Plain `dotnet build`, `dotnet test` and `dotnet run` resolve the same SDK: the product's
Directory.Build.props declares the pinned pair's feed (`rusty status` checks it).

Examples:
  rusty build --project src/Game/Game.csproj
  rusty build --project src/Game/Game.csproj --aot"
        .to_owned()
}

fn install_usage() -> String {
    format!(
        "usage: rusty install [--project <path>] [--archive <pair.tar.gz>]

Installs the Engine SDK/runtime pair pinned by the product (the <{pin}> element in the nearest
{pin_file} at or above --project, default the current directory). It downloads the pair once,
checks its SHA-256 and identity, and keeps it in the shared cache for every product. Installing
never changes the pin. An already installed pair needs no network.

--archive installs a pair archive you already have; its .sha256 file must sit beside it.

Examples:
  rusty install
  rusty install --project ~/dev/my-game
  rusty install --archive ~/Downloads/rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64.tar.gz",
        pin = pair::PIN_ELEMENT,
        pin_file = pair::PIN_FILE,
    )
}

fn update_usage() -> String {
    "usage: rusty update [--project <path>] [--to <version>] [--check]

Moves the product's pin to the newest published Engine pair (or --to an exact version): installs
it, rewrites the pin, and lists the release notes for every pair between the old pin and the new
one. Commit the changed Directory.Build.props with any product changes the notes call for.

  --check  report the newest pair and its notes without installing or changing anything
  --to     choose an exact published version, for example 0.1.0-dev.abc123def456

Examples:
  rusty update --check
  rusty update
  rusty update --to 0.1.0-dev.abc123def456"
        .to_owned()
}

fn status_usage() -> String {
    "usage: rusty status [--project <path>]

Shows the product's pinned pair and where its pin lives, whether that pair is installed, the
runtime pack and SDK feed paths, the pairs in the shared cache, and missing or mismatched
prerequisites. Exits 1 when the product cannot run yet. Works offline.

The project-shape check covers the whole repository, or with --project <product.csproj> only that
project, the projects it references and the .props/.targets files above them. A reference that
follows the pin must be exact; a project that pins Rusty.Engine its own way is listed as a note and
does not affect readiness.

Examples:
  rusty status
  rusty status --project src/Game/Game.csproj"
        .to_owned()
}

#[derive(Debug)]
struct RuntimePack {
    root: PathBuf,
    host: PathBuf,
    manifest: RuntimeManifest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    artifact: String,
    schema_version: u32,
    target: String,
    runtime: Value,
}

impl RuntimePack {
    fn resolve(options: &DevOptions) -> Result<Self, String> {
        let root = if let Some(path) = &options.runtime {
            absolute(path)?
        } else if let Some(source) = &options.engine_source {
            absolute(source)?.join("target/runtime-pack/linux-x64")
        } else {
            runtime_beside_current_executable(&options.project)?
        };
        let manifest_path = root.join("runtime-manifest.json");
        let manifest = fs::read(&manifest_path)
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_RUNTIME_MANIFEST: could not read `{}`: {error}",
                    manifest_path.display()
                )
            })
            .and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|error| {
                    format!(
                        "RUSTY_DEV_RUNTIME_MANIFEST: `{}` is invalid JSON: {error}",
                        manifest_path.display()
                    )
                })
            })?;
        let host = root.join("bin/rusty-product-host");
        Ok(Self {
            root,
            host,
            manifest,
        })
    }

    fn verify(&self) -> Result<(), String> {
        if self.manifest.artifact != "rusty.product.runtime-pack"
            || self.manifest.schema_version != 1
        {
            return Err("RUSTY_DEV_RUNTIME_IDENTITY: runtime manifest must be rusty.product.runtime-pack schemaVersion 1".to_owned());
        }
        if self.manifest.target != "linux-x64" {
            return Err(format!(
                "RUSTY_DEV_RUNTIME_TARGET: runtime pack target `{}` is not supported by this host",
                self.manifest.target
            ));
        }
        if !self.host.is_file() {
            return Err(format!("RUSTY_DEV_RUNTIME_HOST: matched host `{}` is missing; rebuild or select the exact runtime pack", self.host.display()));
        }
        let output = Command::new(&self.host)
            .arg("--identity")
            .output()
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_RUNTIME_HOST: could not execute `{}`: {error}",
                    self.host.display()
                )
            })?;
        if !output.status.success() {
            return Err(format!(
                "RUSTY_DEV_RUNTIME_HOST: `{}` --identity failed with {}",
                self.host.display(),
                output.status
            ));
        }
        let actual: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
            format!("RUSTY_DEV_RUNTIME_IDENTITY: host identity is not valid JSON: {error}")
        })?;
        if actual != self.manifest.runtime {
            return Err("RUSTY_DEV_RUNTIME_IDENTITY: runtime-manifest.json does not match bin/rusty-product-host --identity; select one complete runtime pack or rebuild it".to_owned());
        }
        Ok(())
    }
}

/// An unpinned product run by a runtime pack's own `rusty` uses that pack.
fn runtime_beside_current_executable(project: &Path) -> Result<PathBuf, String> {
    let root = env::current_exe()
        .ok()
        .and_then(|executable| Some(executable.parent()?.parent()?.to_owned()))
        .filter(|root| root.join("runtime-manifest.json").is_file());
    root.ok_or_else(|| {
        format!(
            "{}\nFor Engine contributor work pass --runtime <runtime-pack> or --engine-source <rusty-engine-source>; rusty never searches for an adjacent checkout.",
            require_pin(&product_start(Some(project)).unwrap_or_default())
                .err()
                .unwrap_or_default()
        )
    })
}

/// The directory whose ancestors hold a product's pin: the project's own
/// directory, or the given directory, or the current directory.
fn product_start(project: Option<&Path>) -> Result<PathBuf, String> {
    let path = match project {
        Some(path) => absolute(path)?,
        None => env::current_dir()
            .map_err(|error| format!("RUSTY_PATH: no current directory: {error}"))?,
    };
    if path.is_dir() {
        return Ok(path);
    }
    if path.is_file() {
        return Ok(path.parent().map_or_else(|| path.clone(), Path::to_owned));
    }
    Err(format!("RUSTY_PATH: `{}` does not exist", path.display()))
}

fn require_pin(start: &Path) -> Result<Pin, String> {
    Pin::find(start)?.ok_or_else(|| {
        format!(
            "RUSTY_PIN_MISSING: no <{}> in a {} at or above `{}`. Run from the product repository or pass --project; a product pins its Engine pair with that one element, for example <{}>0.1.0-dev.abc123def456</{}>.",
            pair::PIN_ELEMENT,
            pair::PIN_FILE,
            start.display(),
            pair::PIN_ELEMENT,
            pair::PIN_ELEMENT,
        )
    })
}

/// Tells `build` and `dev` users about project files that let plain dotnet
/// restore a different SDK; the rusty command itself still proceeds.
fn warn_shape(pin: &Pin, project: &Path) {
    match pair::shape_problems(pin, Some(project)) {
        Ok(shape) => shape
            .problems
            .iter()
            .for_each(|problem| eprintln!("RUSTY_PROJECT_SHAPE: {problem}")),
        Err(error) => eprintln!("{error}"),
    }
}

fn not_installed(pin: &Pin) -> String {
    format!(
        "RUSTY_PAIR_NOT_INSTALLED: Engine pair {} (pinned in `{}`) is not installed; run `rusty install` in the product repository.",
        pin.version,
        pin.file.display()
    )
}

/// The product's pinned pair, which must be installed when a pin exists.
fn pinned_pair(project: &Path) -> Result<Option<(Pin, InstalledPair)>, String> {
    let Some(pin) = Pin::find(&product_start(Some(project))?)? else {
        return Ok(None);
    };
    let pair = pair::installed(&pin.version)?.ok_or_else(|| not_installed(&pin))?;
    Ok(Some((pin, pair)))
}

/// The pinned pair when the product has a pin and it is installed; products
/// that bring their own runtime or package source have neither.
fn pinned_pair_if_installed(project: &Path) -> Result<Option<(Pin, InstalledPair)>, String> {
    let Some(pin) = Pin::find(&product_start(Some(project))?)? else {
        return Ok(None);
    };
    Ok(pair::installed(&pin.version)?.map(|pair| (pin, pair)))
}

/// The CoreCLR host locates the runtime through DOTNET_ROOT. A per-user SDK
/// install (for example `~/.dotnet`) is not found without it, so derive it
/// from the `dotnet` the build already uses.
fn use_dotnet_root_for_host() {
    if env::var_os("DOTNET_ROOT").is_some() {
        return;
    }
    if let Some(root) = dotnet_root_from_path() {
        env::set_var("DOTNET_ROOT", root);
    }
}

fn dotnet_root_from_path() -> Option<PathBuf> {
    let dotnet = find_on_path("dotnet")?;
    fs::canonicalize(dotnet).ok()?.parent().map(Path::to_owned)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// `RUSTY_RENDER_OUTPUT=window`: the runtime presents to a native window.
fn window_output() -> bool {
    env::var_os("RUSTY_RENDER_OUTPUT").is_some_and(|value| value == "window")
}

/// Runs the pinned pair's own `rusty dev`, whose supervisor protocol and
/// staging expectations match that pair's host. Returns when this process is
/// already that command.
#[cfg(unix)]
fn delegate_to_pair_cli(runtime: &Path, options: &DevOptions) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let pair_cli = runtime.join("bin/rusty");
    let current = env::current_exe().and_then(fs::canonicalize).ok();
    if current.is_some() && current == fs::canonicalize(&pair_cli).ok() {
        return Ok(());
    }
    let mut command = Command::new(&pair_cli);
    command
        .arg("dev")
        .arg("--project")
        .arg(&options.project)
        .arg("--runtime")
        .arg(runtime);
    if let Some(bind_host) = &options.bind_host {
        command.args(["--bind-host", bind_host]);
    }
    if let Some(port) = options.port {
        command.args(["--port", &port.to_string()]);
    }
    for (enabled, flag) in [
        (options.live_debug, "--live-debug"),
        (options.debugger, "--debugger"),
        (options.headless, "--headless"),
    ] {
        if enabled {
            command.arg(flag);
        }
    }
    let error = command.exec();
    Err(format!(
        "RUSTY_DEV_RUNTIME: could not run the pinned pair's `{}`: {error}; run `rusty install` again if the cache was edited",
        pair_cli.display()
    ))
}

#[cfg(not(unix))]
fn delegate_to_pair_cli(_runtime: &Path, _options: &DevOptions) -> Result<(), String> {
    Ok(())
}

fn build(options: &BuildOptions) -> Result<ExitCode, String> {
    let project = absolute(&options.project)?;
    if !project.is_file() {
        return Err(format!(
            "RUSTY_BUILD_PROJECT: product project `{}` does not exist",
            project.display()
        ));
    }
    if options.engine_source.is_none() {
        if let Some(pin) = Pin::find(&product_start(Some(&project))?)? {
            pair::installed(&pin.version)?.ok_or_else(|| not_installed(&pin))?;
            warn_shape(&pin, &project);
        }
    }
    let mut arguments = vec![
        "msbuild".to_owned(),
        project
            .to_str()
            .ok_or("RUSTY_BUILD_PROJECT: project path must be UTF-8")?
            .to_owned(),
        "-restore".to_owned(),
        "-nologo".to_owned(),
        "-verbosity:minimal".to_owned(),
        format!(
            "-t:{}",
            if options.aot {
                AOT_TARGET
            } else {
                STAGE_TARGET
            }
        ),
    ];
    arguments.extend(source_properties(options.engine_source.as_deref())?);
    let status = Command::new("dotnet")
        .args(&arguments)
        .status()
        .map_err(|error| format!("RUSTY_PREREQUISITE: could not start dotnet: {error}; install the .NET {REQUIRED_DOTNET_MAJOR} SDK"))?;
    if status.success() {
        return Ok(ExitCode::SUCCESS);
    }
    eprintln!(
        "RUSTY_BUILD: dotnet {} failed with {status}",
        arguments.join(" ")
    );
    Ok(exit_code(status))
}

fn exit_code(status: ExitStatus) -> ExitCode {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .filter(|code| *code != 0)
        .map_or(ExitCode::FAILURE, ExitCode::from)
}

fn install(options: &InstallOptions) -> Result<ExitCode, String> {
    if let Some(archive) = &options.archive {
        let pair = pair::install_archive(&absolute(archive)?)?;
        println!(
            "Installed Engine pair {} at {}",
            pair.version,
            pair.root.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    let pin = require_pin(&product_start(options.project.as_deref())?)?;
    let (pair, downloaded) = pair::install_version(&pin.version)?;
    if downloaded {
        println!(
            "Installed Engine pair {} at {}",
            pair.version,
            pair.root.display()
        );
    } else {
        println!(
            "Engine pair {} is already installed at {}",
            pair.version,
            pair.root.display()
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn update(options: &UpdateOptions) -> Result<ExitCode, String> {
    let pin = require_pin(&product_start(options.project.as_deref())?)?;
    let (target_version, target) = match &options.to {
        Some(version) => (version.clone(), pair::release(version)?),
        None => {
            let latest = pair::latest_release()?;
            let version = latest["version"]
                .as_str()
                .ok_or("RUSTY_RELEASE_METADATA: the latest pair-release.json names no version")?
                .to_owned();
            pair::validate_version(&version)?;
            (version, Some(latest))
        }
    };
    println!("pinned   {} ({})", pin.version, pin.file.display());
    println!("target   {target_version}");
    if target_version == pin.version {
        println!("The product is already pinned to {target_version}.");
        return Ok(ExitCode::SUCCESS);
    }
    let notes = match &target {
        Some(release) => pair::release_notes_chain(release, &pin.version)?,
        None => vec![format!(
            "  {target_version}: published without release information"
        )],
    };
    if options.check {
        println!("An update is available. What changed:");
        notes.iter().for_each(|line| println!("{line}"));
        let to = options
            .to
            .as_ref()
            .map_or_else(String::new, |version| format!(" --to {version}"));
        println!("Apply it with: rusty update{to}");
        return Ok(ExitCode::SUCCESS);
    }
    let (pair, _) = pair::install_version(&target_version)?;
    pin.write(&target_version)?;
    println!(
        "Pinned {} -> {target_version} in {}; installed at {}",
        pin.version,
        pin.file.display(),
        pair.root.display()
    );
    println!("What changed (read each before building):");
    notes.iter().for_each(|line| println!("{line}"));
    println!(
        "Next: rebuild and run the product (rusty build / rusty dev), then commit {}.",
        pin.file.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn status(options: &StatusOptions) -> Result<ExitCode, String> {
    let start = product_start(options.project.as_deref())?;
    let mut problems = Vec::new();
    let mut needs_download = false;
    match Pin::find(&start)? {
        Some(pin) => {
            println!("pin            {} ({})", pin.version, pin.file.display());
            match pair::installed(&pin.version)? {
                Some(pair) => {
                    println!("installed      yes");
                    println!("runtime pack   {}", pair.runtime_pack().display());
                    let desktop = pair.desktop_pack();
                    if desktop.join("runtime-manifest.json").is_file() {
                        println!("desktop pack   {}", desktop.display());
                    } else {
                        println!("desktop pack   not installed (fetched on the first RUSTY_RENDER_OUTPUT=window run)");
                    }
                    println!("sdk feed       {}", pair.sdk_feed().display());
                }
                None => {
                    println!("installed      no");
                    needs_download = true;
                    problems.push(format!(
                        "pair {} is not installed: run `rusty install`",
                        pin.version
                    ));
                }
            }
            let selected = match &options.project {
                Some(project)
                    if project
                        .extension()
                        .is_some_and(|extension| extension == "csproj") =>
                {
                    Some(absolute(project)?)
                }
                _ => None,
            };
            let shape = pair::shape_problems(&pin, selected.as_deref())?;
            println!(
                "project shape  {}",
                if shape.problems.is_empty() {
                    "exact pin, pair feed declared"
                } else {
                    "needs changes"
                }
            );
            for note in &shape.notes {
                println!("               note: {note}");
            }
            problems.extend(shape.problems);
        }
        None => {
            println!("pin            none at or above {}", start.display());
            problems.push(format!(
                "no pin: add <{}> to the product's {}",
                pair::PIN_ELEMENT,
                pair::PIN_FILE
            ));
        }
    }
    let cache = pair::cache_root()?;
    let versions = pair::installed_versions()?;
    println!(
        "cache          {} ({} pairs)",
        cache.display(),
        versions.len()
    );
    for version in &versions {
        println!("               {version}");
    }
    if let Ok(executable) = env::current_exe() {
        println!("this rusty     {}", executable.display());
    }

    match find_on_path("dotnet") {
        None => {
            println!("dotnet         missing");
            problems.push(format!(
                "install the .NET {REQUIRED_DOTNET_MAJOR} SDK and put dotnet on PATH"
            ));
        }
        Some(path) => {
            let version = Command::new(&path)
                .arg("--version")
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
            let major = version
                .as_deref()
                .and_then(|version| version.split('.').next())
                .and_then(|major| major.parse::<u32>().ok());
            println!(
                "dotnet         {} ({})",
                version.as_deref().unwrap_or("unknown version"),
                path.display()
            );
            if major.is_none_or(|major| major < REQUIRED_DOTNET_MAJOR) {
                problems.push(format!(
                    "dotnet SDK {} is older than the required {REQUIRED_DOTNET_MAJOR}",
                    version.as_deref().unwrap_or("of unknown version")
                ));
            }
        }
    }
    match env::var_os("DOTNET_ROOT") {
        Some(root) => println!("DOTNET_ROOT    {}", PathBuf::from(root).display()),
        None => match dotnet_root_from_path() {
            Some(root) => println!("DOTNET_ROOT    unset; rusty dev uses {}", root.display()),
            None => println!("DOTNET_ROOT    unset"),
        },
    }
    for tool in ["curl", "tar"] {
        let found = find_on_path(tool);
        println!(
            "{tool:<15}{}",
            found.as_ref().map_or_else(
                || "missing (install and update need it)".to_owned(),
                |path| path.display().to_string()
            )
        );
        if found.is_none() && needs_download {
            problems.push(format!("install {tool}, which `rusty install` needs"));
        }
    }

    if problems.is_empty() {
        println!("ready          yes");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("ready          no");
        for problem in &problems {
            println!("  - {problem}");
        }
        Ok(ExitCode::FAILURE)
    }
}

struct StagedProduct {
    directory: PathBuf,
    watches: Vec<PathBuf>,
    /// Edits wholly under these roots take the asset-only path: the UI source
    /// and output roots, and each content bundle root. Loose content is part
    /// of the product's create-time snapshot, so it is not an asset root.
    asset_roots: Vec<PathBuf>,
}

/// Restore, build, stage and read the staged directory and watch declaration
/// in one MSBuild invocation. Build output stays on the console; the evaluated
/// properties are written to a result file.
fn stage_product(options: &DevOptions) -> Result<StagedProduct, String> {
    let project = absolute(&options.project)?;
    if !project.is_file() {
        return Err(format!(
            "RUSTY_DEV_PROJECT: ordinary product project `{}` does not exist",
            project.display()
        ));
    }
    let project_argument = project
        .to_str()
        .ok_or("RUSTY_DEV_PROJECT: project path must be UTF-8")?
        .to_owned();
    let result_file = env::temp_dir().join(format!("rusty-dev-stage-{}.json", std::process::id()));
    let result_argument = result_file
        .to_str()
        .ok_or("RUSTY_DEV_STAGE: temporary result path must be UTF-8")?
        .to_owned();
    // `-restore` restores in its own evaluation and re-evaluates before the
    // target, so a fresh or package-changed project imports the SDK's staging
    // targets. Restore is incremental when its inputs are unchanged.
    let mut arguments = vec![
        "msbuild".to_owned(),
        project_argument,
        "-restore".to_owned(),
        "-nologo".to_owned(),
        "-verbosity:minimal".to_owned(),
        format!("-t:{STAGE_TARGET}"),
    ];
    arguments.extend(stage_properties(options)?);
    for property in [
        STAGED_PRODUCT_PROPERTY,
        WATCH_PATHS_PROPERTY,
        UI_SOURCE_ROOT_PROPERTY,
        UI_ROOT_PROPERTY,
        CONTENT_ROOT_PROPERTY,
    ] {
        arguments.push(format!("-getProperty:{property}"));
    }
    arguments.push(format!("-getItem:{CONTENT_BUNDLE_ITEM}"));
    arguments.push(format!("-getResultOutputFile:{result_argument}"));
    let _ = fs::remove_file(&result_file);
    run_dotnet(&arguments)?;
    let result = fs::read(&result_file).map_err(|error| {
        format!("RUSTY_DEV_MSBUILD: staging produced no property result: {error}")
    });
    let _ = fs::remove_file(&result_file);
    let result: Value = serde_json::from_slice(&result?).map_err(|error| {
        format!("RUSTY_DEV_MSBUILD: staging property result is not JSON: {error}")
    })?;
    let property = |name: &str| -> Result<&str, String> {
        result["Properties"][name]
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("RUSTY_DEV_MSBUILD: property {name} produced no value"))
    };
    Ok(StagedProduct {
        directory: absolute(Path::new(property(STAGED_PRODUCT_PROPERTY)?.trim()))?,
        watches: parse_watch_paths(property(WATCH_PATHS_PROPERTY)?)?,
        asset_roots: asset_roots(&project, &result)?,
    })
}

fn asset_roots(project: &Path, result: &Value) -> Result<Vec<PathBuf>, String> {
    let project_directory = project.parent().unwrap_or(project);
    let path = |name: &str| -> Option<PathBuf> {
        result["Properties"][name]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| project_directory.join(value))
    };
    let mut roots: Vec<PathBuf> = [UI_SOURCE_ROOT_PROPERTY, UI_ROOT_PROPERTY]
        .into_iter()
        .filter_map(path)
        .collect();
    if let Some(content_root) = path(CONTENT_ROOT_PROPERTY) {
        let bundles = result["Items"][CONTENT_BUNDLE_ITEM].as_array();
        for bundle in bundles.into_iter().flatten() {
            let root = bundle["Root"]
                .as_str()
                .filter(|root| !root.is_empty())
                .or_else(|| bundle["Identity"].as_str())
                .ok_or("RUSTY_DEV_MSBUILD: content bundle item has no identity")?;
            roots.push(content_root.join(root));
        }
    }
    Ok(roots)
}

/// Restage only UI and content through the SDK asset target. The project was
/// restored by the full staging that preceded it.
fn stage_assets(options: &DevOptions) -> Result<(), String> {
    let project = absolute(&options.project)?;
    let mut arguments = vec![
        "msbuild".to_owned(),
        project
            .to_str()
            .ok_or("RUSTY_DEV_PROJECT: project path must be UTF-8")?
            .to_owned(),
        "-nologo".to_owned(),
        "-verbosity:minimal".to_owned(),
        format!("-t:{STAGE_ASSETS_TARGET}"),
    ];
    arguments.extend(stage_properties(options)?);
    run_dotnet(&arguments)
}

fn parse_watch_paths(value: &str) -> Result<Vec<PathBuf>, String> {
    let paths = value
        .split(';')
        .filter(|value| !value.trim().is_empty())
        .map(|value| absolute(Path::new(value.trim())))
        .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() {
        return Err(format!("RUSTY_DEV_WATCH_DECLARATION: SDK property {WATCH_PATHS_PROPERTY} was empty; declare the C#/UI/content inputs in the product project"));
    }
    for path in &paths {
        if !path.exists() {
            return Err(format!(
                "RUSTY_DEV_WATCH_DECLARATION: declared watch path `{}` does not exist",
                path.display()
            ));
        }
    }
    Ok(paths)
}

fn source_properties(engine_source: Option<&Path>) -> Result<Vec<String>, String> {
    let Some(engine_source) = engine_source else {
        return Ok(Vec::new());
    };
    let engine_source = absolute(engine_source)?;
    let engine_source = engine_source
        .to_str()
        .ok_or("RUSTY_DEV_ENGINE_SOURCE: Engine source path must be UTF-8")?;
    Ok(vec![
        "-p:RustyEngineUseSourceDevelopment=true".to_owned(),
        format!("-p:RustyEngineSourceDevelopmentPath={engine_source}"),
    ])
}

fn stage_properties(options: &DevOptions) -> Result<Vec<String>, String> {
    let mut properties = source_properties(options.engine_source.as_deref())?;
    if let Some(bind_host) = &options.bind_host {
        properties.push(format!("-p:RustyEngineProductBindHost={bind_host}"));
    }
    if let Some(port) = options.port {
        properties.push(format!("-p:RustyEngineProductPort={port}"));
    }
    if options.live_debug {
        properties.push("-p:RustyEngineProductLiveDebug=true".to_owned());
    }
    Ok(properties)
}

fn run_dotnet(arguments: &[String]) -> Result<(), String> {
    let status = Command::new("dotnet")
        .args(arguments)
        .status()
        .map_err(|error| format!("RUSTY_DEV_DOTNET: could not start dotnet: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "RUSTY_DEV_BUILD: dotnet {} failed with {status}",
            arguments.join(" ")
        ))
    }
}

fn verify_staged_product(staged: &Path) -> Result<(), String> {
    let manifest = staged.join("product.json");
    if manifest.is_file() {
        Ok(())
    } else {
        Err(format!("RUSTY_DEV_STAGE: SDK target {STAGE_TARGET} reported `{}`, but its atomically staged product.json is missing", staged.display()))
    }
}

fn development_persistence_root(project: &Path) -> Result<PathBuf, String> {
    Ok(development_runtime_root(project)?.join("persistence"))
}

fn development_runtime_root(project: &Path) -> Result<PathBuf, String> {
    let project = absolute(project)?;
    let project_directory = project.parent().ok_or_else(|| {
        format!(
            "RUSTY_DEV_PROJECT: ordinary product project `{}` has no containing directory",
            project.display()
        )
    })?;
    // Keep developer state beside the Product repository when one is
    // discoverable. This matches the ordinary `.runtime` lane used by
    // downstream products and avoids writing state beneath a source project
    // directory that may not ignore generated files. A loose project still
    // gets a stable root beside its project file.
    let product_root = project_directory
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .unwrap_or(project_directory);
    Ok(product_root.join(".runtime"))
}

struct SupervisedHost {
    child: Child,
    stdin: Option<ChildStdin>,
    runtime_instance_id: u64,
}

impl SupervisedHost {
    fn start(
        host: &Path,
        product: &Path,
        persistence_root: &Path,
        debugger: bool,
        headless: bool,
    ) -> Result<Self, String> {
        let runtime_instance_id = next_supervised_runtime_instance_id()?;
        let arguments = supervised_host_arguments(
            product,
            persistence_root,
            runtime_instance_id,
            debugger,
            headless,
        )?;
        let mut child = Command::new(host)
            .args(&arguments)
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_CHILD_START: could not launch `{}`: {error}",
                    host.display()
                )
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or("RUSTY_DEV_CHILD_START: supervised child stdin was unavailable")?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            runtime_instance_id,
        })
    }

    fn shutdown(&mut self) -> Result<(), String> {
        // Closing the existing supervision pipe asks the host to drain its
        // runtime and product disposal. Keep inherited output open until reaped.
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.try_wait()? {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "RUSTY_DEV_CHILD_SHUTDOWN: host exited with {status}"
                    ))
                };
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err("RUSTY_DEV_CHILD_SHUTDOWN_TIMEOUT: host did not finish disposal within 30 seconds".into());
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("RUSTY_DEV_CHILD_WAIT: {error}"))
    }

    /// Replaces only the disposable runtime inside a live supervised shell.
    /// The browser listener, diagnostics, and persistent Engine roots remain
    /// fixed process configuration for the duration of `rusty dev`.
    fn replace_runtime(&mut self, product: &Path) -> Result<(), String> {
        let product_directory = product
            .to_str()
            .ok_or("RUSTY_DEV_STAGE: staged product path must be UTF-8")?
            .to_owned();
        self.send(&SupervisedHostCommand::ReplaceRuntime { product_directory })
    }

    /// Asks the running product to re-read its restaged UI and content.
    fn reload_assets(&mut self) -> Result<(), String> {
        self.send(&SupervisedHostCommand::ReloadAssets)
    }

    fn send(&mut self, command: &SupervisedHostCommand) -> Result<(), String> {
        let frame = encode_supervisor_command(command)?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or("RUSTY_DEV_CHILD_COMMAND: supervised child stdin was unavailable")?;
        stdin
            .write_all(&frame)
            .and_then(|_| stdin.flush())
            .map_err(|error| format!("RUSTY_DEV_CHILD_COMMAND: could not send command: {error}"))
    }
}

/// The commands the long-lived `rusty dev` shell accepts after startup. They
/// carry no generic method name, options bag, or compatibility negotiation:
/// successful staging either replaces exactly one C# runtime incarnation with
/// the next staged Product directory, or reloads restaged UI and content into
/// the running one.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SupervisedHostCommand {
    #[serde(rename_all = "camelCase")]
    ReplaceRuntime {
        product_directory: String,
    },
    ReloadAssets,
}

fn encode_supervisor_command(command: &SupervisedHostCommand) -> Result<Vec<u8>, String> {
    let payload = serde_json::to_vec(command).map_err(|error| {
        format!("RUSTY_DEV_CHILD_COMMAND: command could not be encoded: {error}")
    })?;
    if payload.len() > MAX_SUPERVISOR_COMMAND_BYTES {
        return Err(
            "RUSTY_DEV_CHILD_COMMAND: command exceeds its bounded command length".to_owned(),
        );
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| "RUSTY_DEV_CHILD_COMMAND: command length cannot be represented".to_owned())?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn supervised_host_arguments(
    product: &Path,
    persistence_root: &Path,
    runtime_instance_id: u64,
    debugger: bool,
    headless: bool,
) -> Result<Vec<String>, String> {
    if runtime_instance_id == 0 {
        return Err(
            "RUSTY_DEV_RUNTIME_INSTANCE: runtime incarnation identity must be nonzero".to_owned(),
        );
    }
    let mut arguments = vec![
        "--product".to_owned(),
        product
            .to_str()
            .ok_or("RUSTY_DEV_STAGE: staged product path must be UTF-8")?
            .to_owned(),
        "--loader".to_owned(),
        "coreclr".to_owned(),
        "--supervised".to_owned(),
        "--runtime-instance-id".to_owned(),
        runtime_instance_id.to_string(),
        "--persistence-root".to_owned(),
        persistence_root
            .to_str()
            .ok_or("RUSTY_DEV_PERSISTENCE: persistence root path must be UTF-8")?
            .to_owned(),
    ];
    if debugger {
        arguments.push("--debugger".to_owned());
    }
    if headless {
        arguments.push("--headless".to_owned());
    }
    Ok(arguments)
}

fn next_supervised_runtime_instance_id() -> Result<u64, String> {
    let ordinal = NEXT_SUPERVISED_RUNTIME_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    let process_seed = u64::from(std::process::id()).max(1);
    process_seed.checked_add(ordinal).ok_or_else(|| {
        "RUSTY_DEV_RUNTIME_INSTANCE: supervisor incarnation identity space exhausted".to_owned()
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileSnapshot(BTreeMap<PathBuf, FileStamp>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    bytes: u64,
}

impl FileSnapshot {
    fn capture(paths: &[PathBuf]) -> Result<Self, String> {
        let mut files = BTreeMap::new();
        for path in paths {
            capture_path(path, &mut files)?;
        }
        Ok(Self(files))
    }

    /// Files added, removed or changed between this snapshot and `next`.
    fn changed_paths(&self, next: &Self) -> Vec<PathBuf> {
        let removed_or_changed = self
            .0
            .iter()
            .filter(|(path, stamp)| next.0.get(*path) != Some(stamp))
            .map(|(path, _)| path.clone());
        let added = next
            .0
            .keys()
            .filter(|path| !self.0.contains_key(*path))
            .cloned();
        removed_or_changed.chain(added).collect()
    }
}

/// Records an observed source state before trying to stage it. If staging is
/// broken, the watcher retains the known-good runtime but does not rebuild the
/// identical broken state every polling interval; a subsequent edit differs
/// and is admitted as the next restage attempt.
fn record_observed_snapshot(current: &mut FileSnapshot, observed: FileSnapshot) -> bool {
    if *current == observed {
        return false;
    }
    *current = observed;
    true
}

fn capture_path(path: &Path, files: &mut BTreeMap<PathBuf, FileStamp>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "RUSTY_DEV_WATCH: could not inspect `{}`: {error}",
            path.display()
        )
    })?;
    if metadata.is_file() {
        files.insert(
            path.to_owned(),
            FileStamp {
                modified: metadata.modified().ok(),
                bytes: metadata.len(),
            },
        );
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| {
            format!(
                "RUSTY_DEV_WATCH: could not read `{}`: {error}",
                path.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "RUSTY_DEV_WATCH: could not enumerate `{}`: {error}",
                    path.display()
                )
            })?;
            let child = entry.path();
            if ignored_watch_directory(&child) {
                continue;
            }
            capture_path(&child, files)?;
        }
    }
    Ok(())
}

fn ignored_watch_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_WATCH_DIRECTORY_NAMES.contains(&name))
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        env::current_dir()
            .map(|root| root.join(path))
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_PATH: could not resolve `{}`: {error}",
                    path.display()
                )
            })
    }
}

fn diagnostic(event: &str, detail: Value) {
    println!(
        "RUSTY_DEV {}",
        serde_json::json!({ "schemaVersion": 1, "event": event, "detail": detail })
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_budget_restarts_once_then_pauses_for_the_same_artifact() {
        let mut budget = CrashBudget::new(2);
        assert_eq!(
            budget.record_unexpected_exit(),
            CrashBudgetDecision::Restart {
                backoff: UNEXPECTED_EXIT_RESTART_BACKOFF
            }
        );
        assert_eq!(
            budget.record_unexpected_exit(),
            CrashBudgetDecision::PausedFault
        );
        assert_eq!(budget.unexpected_exit_count(), 2);
    }

    #[test]
    fn successful_restage_resets_a_paused_crash_budget() {
        let mut budget = CrashBudget::new(2);
        let _ = budget.record_unexpected_exit();
        assert_eq!(
            budget.record_unexpected_exit(),
            CrashBudgetDecision::PausedFault
        );
        budget.reset_after_successful_restage();
        assert_eq!(budget.unexpected_exit_count(), 0);
        assert!(matches!(
            budget.record_unexpected_exit(),
            CrashBudgetDecision::Restart { .. }
        ));
    }

    #[test]
    fn observed_broken_snapshot_is_recorded_until_a_later_edit_changes_it() {
        let path = PathBuf::from("/workspace/Product/Program.cs");
        let snapshot = |bytes| {
            FileSnapshot(BTreeMap::from([(
                path.clone(),
                FileStamp {
                    modified: None,
                    bytes,
                },
            )]))
        };
        let mut recorded = snapshot(10);
        let broken = snapshot(20);

        assert!(record_observed_snapshot(&mut recorded, broken.clone()));
        assert_eq!(recorded, broken);
        assert!(
            !record_observed_snapshot(&mut recorded, broken),
            "the unchanged broken state must not restage again"
        );
        assert!(
            record_observed_snapshot(&mut recorded, snapshot(30)),
            "a correcting source edit gets a fresh restage attempt"
        );
    }

    #[test]
    fn debugger_mode_reaches_supervised_host_without_changing_product_staging() {
        let arguments = Arguments::parse(
            ["dev", "--project", "Product.csproj", "--debugger"].map(str::to_owned),
        )
        .expect("debugger options");
        let CommandName::Dev(options) = arguments.command else {
            panic!("dev command");
        };
        assert!(options.debugger);
        assert!(!stage_properties(&options)
            .expect("staging properties")
            .iter()
            .any(|property| property.contains("debugger")));
        let host = supervised_host_arguments(
            Path::new("/product"),
            Path::new("/persistence"),
            7,
            options.debugger,
            options.headless,
        )
        .expect("host arguments");
        assert!(host.iter().any(|argument| argument == "--debugger"));
    }

    #[test]
    fn headless_option_reaches_the_supervised_host() {
        let arguments = Arguments::parse(
            ["dev", "--project", "Product.csproj", "--headless"].map(str::to_owned),
        )
        .expect("headless options");
        let CommandName::Dev(options) = arguments.command else {
            panic!("dev command");
        };
        assert!(options.headless);
        let host = supervised_host_arguments(
            Path::new("/product"),
            Path::new("/persistence"),
            7,
            options.debugger,
            options.headless,
        )
        .expect("host arguments");
        assert!(host.iter().any(|argument| argument == "--headless"));
    }

    #[test]
    fn dev_options_are_explicit_and_coreclr_scoped() {
        let arguments = Arguments::parse([
            "dev".to_owned(),
            "--project".to_owned(),
            "Product.csproj".to_owned(),
            "--runtime".to_owned(),
            "/runtime-pack".to_owned(),
            "--bind-host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            "9348".to_owned(),
            "--live-debug".to_owned(),
        ])
        .expect("dev options parse");
        let CommandName::Dev(options) = arguments.command else {
            panic!("dev command");
        };
        assert_eq!(options.project, PathBuf::from("Product.csproj"));
        assert_eq!(options.runtime, Some(PathBuf::from("/runtime-pack")));
        assert_eq!(options.bind_host.as_deref(), Some("127.0.0.1"));
        assert_eq!(options.port, Some(9348));
        assert!(options.live_debug);
        assert!(!options.debugger);
        assert!(!options.headless);
    }

    #[test]
    fn runtime_and_source_overrides_cannot_be_combined() {
        let error = Arguments::parse([
            "dev".to_owned(),
            "--project".to_owned(),
            "Product.csproj".to_owned(),
            "--runtime".to_owned(),
            "/runtime-pack".to_owned(),
            "--engine-source".to_owned(),
            "/engine".to_owned(),
        ])
        .expect_err("ambiguous runtime source is rejected");
        assert!(error.contains("mutually exclusive"));
    }

    #[test]
    fn engine_source_is_forwarded_to_sdk_build_and_staging() {
        let options = DevOptions {
            project: PathBuf::from("Product.csproj"),
            runtime: None,
            engine_source: Some(PathBuf::from("/engine-source")),
            bind_host: None,
            port: None,
            live_debug: false,
            debugger: false,
            headless: false,
        };

        let properties = stage_properties(&options).expect("source properties");
        assert!(properties.contains(&"-p:RustyEngineUseSourceDevelopment=true".to_owned()));
        assert!(
            properties.contains(&"-p:RustyEngineSourceDevelopmentPath=/engine-source".to_owned())
        );
    }

    #[test]
    fn development_runtime_roots_are_repository_local_and_absolute() {
        let persistence_root = development_persistence_root(Path::new("src/Product.csproj"))
            .expect("development persistence root");
        let current_directory = env::current_dir().expect("current directory");
        let repository_root = current_directory
            .ancestors()
            .find(|candidate| candidate.join(".git").exists())
            .expect("test runs from the Engine repository");

        assert_eq!(
            persistence_root,
            repository_root.join(".runtime").join("persistence")
        );
        assert!(persistence_root.is_absolute());
    }

    #[test]
    fn development_runtime_roots_for_a_loose_project_do_not_follow_staged_output() {
        let persistence_root =
            development_persistence_root(Path::new("/workspace/Product/Product.csproj"))
                .expect("development persistence root");
        let staged_product = Path::new("/workspace/Product/obj/RustyEngineProduct");

        assert_eq!(
            persistence_root,
            PathBuf::from("/workspace/Product/.runtime/persistence")
        );
        assert_ne!(
            persistence_root,
            staged_product.join(".runtime/persistence")
        );
    }

    #[test]
    fn supervised_host_arguments_include_distinct_stable_runtime_roots() {
        let arguments = supervised_host_arguments(
            Path::new("/workspace/Product/obj/RustyEngineProduct"),
            Path::new("/workspace/Product/.runtime/persistence"),
            41,
            false,
            false,
        )
        .expect("supervised host arguments");

        let expected = [
            "--product",
            "/workspace/Product/obj/RustyEngineProduct",
            "--loader",
            "coreclr",
            "--supervised",
            "--runtime-instance-id",
            "41",
            "--persistence-root",
            "/workspace/Product/.runtime/persistence",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert_eq!(arguments, expected);
    }

    #[test]
    fn restage_encodes_the_one_bounded_runtime_replacement_command() {
        let frame = encode_supervisor_command(&SupervisedHostCommand::ReplaceRuntime {
            product_directory: "/workspace/Product/obj/RustyEngineProduct".to_owned(),
        })
        .expect("replacement frame");

        let length = u32::from_le_bytes(frame[..4].try_into().expect("frame prefix")) as usize;
        assert_eq!(length, frame.len() - 4);
        let payload: Value = serde_json::from_slice(&frame[4..]).expect("replacement payload");
        assert_eq!(
            payload,
            serde_json::json!({
                "kind": "replace-runtime",
                "productDirectory": "/workspace/Product/obj/RustyEngineProduct",
            })
        );
    }

    #[test]
    fn asset_reload_encodes_a_bare_command() {
        let frame =
            encode_supervisor_command(&SupervisedHostCommand::ReloadAssets).expect("reload frame");
        let payload: Value = serde_json::from_slice(&frame[4..]).expect("reload payload");
        assert_eq!(payload, serde_json::json!({ "kind": "reload-assets" }));
    }

    #[test]
    fn snapshot_diff_reports_added_removed_and_changed_files() {
        let stamp = |bytes| FileStamp {
            modified: None,
            bytes,
        };
        let before = FileSnapshot(BTreeMap::from([
            (PathBuf::from("/p/kept"), stamp(1)),
            (PathBuf::from("/p/edited"), stamp(1)),
            (PathBuf::from("/p/deleted"), stamp(1)),
        ]));
        let after = FileSnapshot(BTreeMap::from([
            (PathBuf::from("/p/kept"), stamp(1)),
            (PathBuf::from("/p/edited"), stamp(2)),
            (PathBuf::from("/p/added"), stamp(1)),
        ]));
        let mut changed = before.changed_paths(&after);
        changed.sort();
        assert_eq!(
            changed,
            ["/p/added", "/p/deleted", "/p/edited"].map(PathBuf::from)
        );
    }

    #[test]
    fn asset_roots_are_ui_roots_and_content_bundle_roots_only() {
        let result = serde_json::json!({
            "Properties": {
                "RustyEngineProductUiSourceRoot": "/product/ui",
                "RustyEngineProductUiRoot": "/product/ui/generated",
                "RustyEngineProductContentRoot": "/product/content",
            },
            "Items": {
                "RustyEngineContentBundle": [
                    { "Identity": "procgen", "Root": "" },
                    { "Identity": "music", "Root": "media/music" },
                ],
            },
        });
        let roots = asset_roots(Path::new("/product/src/Game.csproj"), &result).unwrap();
        assert_eq!(
            roots,
            [
                "/product/ui",
                "/product/ui/generated",
                "/product/content/procgen",
                "/product/content/media/music",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn initial_supervised_shell_rejects_a_zero_runtime_seed() {
        let error = supervised_host_arguments(
            Path::new("/workspace/Product"),
            Path::new("/workspace/Product/.runtime/persistence"),
            0,
            false,
            false,
        )
        .expect_err("zero runtime incarnation is not a valid shell seed");
        assert!(error.contains("must be nonzero"));
    }

    #[test]
    fn supervisor_allocates_distinct_nonzero_runtime_incarnations() {
        let first = next_supervised_runtime_instance_id().expect("first runtime incarnation");
        let second = next_supervised_runtime_instance_id().expect("second runtime incarnation");

        assert_ne!(first, 0);
        assert_ne!(second, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn generated_and_tool_owned_directories_are_excluded_from_source_watches() {
        for name in IGNORED_WATCH_DIRECTORY_NAMES {
            assert!(
                ignored_watch_directory(Path::new("/product/src").join(name).as_path()),
                "{name} must not restart rusty dev"
            );
        }
        for name in ["Modules", "content", "ui", "Shaders"] {
            assert!(
                !ignored_watch_directory(Path::new("/product/src").join(name).as_path()),
                "{name} remains an authored source directory"
            );
        }
    }
}
