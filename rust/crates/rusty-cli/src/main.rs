//! The `rusty` command: the downstream product workflow for Rusty Engine.
//!
//! It installs and updates the product's pinned SDK/runtime pair, reports what
//! is selected and missing, and builds and runs the product. It holds no
//! Product configuration: the SDK evaluates and stages that truth, and the
//! pinned runtime pack supplies the exact host that `rusty dev` starts.

mod asset;
mod pair;
mod session;

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
use session::{Registration, Session};

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
#[cfg(not(windows))]
const BOOTSTRAP: &str =
    "curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash";
#[cfg(windows)]
const BOOTSTRAP: &str =
    "irm https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.ps1 | iex";
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const UNEXPECTED_EXIT_RESTART_BACKOFF: Duration = Duration::from_millis(100);
/// `rusty-product-host` exits with this code when it cannot bind its port
/// (`BIND_FAILURE_EXIT_CODE` in csharp-product-runtime's supervisor; the two
/// ship in one pair).
const HOST_BIND_FAILURE_EXIT_CODE: i32 = 78;
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
        CommandName::DevHelp(project) => {
            if let Some(pair_cli) = pinned_pair_cli(project.as_deref()) {
                delegate_help_to_pair_cli(&pair_cli)?;
            }
            println!("{}", dev_usage());
            Ok(ExitCode::SUCCESS)
        }
        CommandName::Dev(options) => dev(options).map(|()| ExitCode::SUCCESS),
        CommandName::DevStart(options, arguments) => {
            check_port(&options)?;
            session::start(&options.project, options.instance.as_deref(), &arguments)
        }
        CommandName::DevStop(project, instance) => session::stop(&project, instance.as_deref()),
        CommandName::DevStopTarget(target) => session::stop_target(&target),
        CommandName::DevList { json, all } => session::list(json, all),
        CommandName::DevKeep { target, keep } => session::keep(&target, keep),
        CommandName::DevPrune => session::prune(),
        CommandName::DevStatus(project, instance) => session::status(&project, instance.as_deref()),
        CommandName::Build(options) => build(&options),
        CommandName::Install(options) => install(&options),
        CommandName::Update(options) => update(&options),
        CommandName::Status(options) => status(&options),
        CommandName::PackContent(options) => pack_content(&options),
        CommandName::AssetCheck(path) => asset_check(&path),
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
    let pinned_selection = pinned_selection(&options);
    if options.engine_source.is_none() {
        let pinned = if options.runtime.is_none() {
            pinned_pair(&options.project)?
        } else {
            pinned_pair_if_installed(&options.project)?
        };
        if let Some((pin, pair)) = pinned {
            warn_shape(&pin, &absolute(&options.project)?);
            if options.runtime.is_none() {
                // Window output switches to the pair's desktop pack once the
                // staged manifest says so (see `window_runtime`).
                let runtime = pair.runtime_pack();
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
    check_port(&options)?;
    let roots = DevelopmentRoots::of(&options.project)?;
    let persistence_root = roots.persistence();
    let session = Session::claim(&Registration {
        project: &options.project,
        persistence_root: &persistence_root,
        instance: options.instance.as_deref(),
        checkout: &roots.checkout,
        background: options.session,
        label: options.label.as_deref(),
        keep: options.keep,
    })?;
    let session = Some(&session);
    let termination = install_termination_signal_hook()?;
    let runtime = RuntimePack::resolve(&options)?;
    runtime.verify()?;
    if let Some(session) = session {
        session.uses_runtime(&runtime.root);
    }

    let initial = stage_product(&options)?;
    let mut staged = initial.directory;
    verify_staged_product(&staged)?;
    let window = window_output(&staged)?;
    let runtime = window_runtime(runtime, &staged, pinned_selection)?;
    let idle_limit = idle_limit(&options, window);
    if let Some(session) = session {
        session.idle_limit(&idle_limit);
    }
    let mut watches = initial.watches;
    let mut asset_roots = initial.asset_roots;
    let mut snapshot = FileSnapshot::capture(&watches)?;
    let mut last_capture = Duration::ZERO;
    let mut child = Some(SupervisedHost::start(
        &runtime.host,
        &staged,
        &persistence_root,
        &options,
        session,
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
        let stop_requested = session.is_some_and(|session| session.stop_requested());
        if termination.load(Ordering::Acquire) || stop_requested {
            if let Some(mut active_child) = child.take() {
                active_child.shutdown()?;
            }
            let reason = if stop_requested {
                "stop-requested"
            } else {
                "termination-signal"
            };
            diagnostic("stopped", serde_json::json!({ "reason": reason }));
            return Ok(());
        }
        if let (Ok(limit), Some(session)) = (&idle_limit, session) {
            if !session.kept() && session.idle() >= *limit {
                if let Some(mut active_child) = child.take() {
                    active_child.shutdown()?;
                }
                session.finish("idle-expired");
                diagnostic(
                    "stopped",
                    serde_json::json!({
                        "reason": "idle-expired",
                        "idleTimeoutMinutes": limit.as_secs() / 60,
                    }),
                );
                eprintln!(
                    "rusty: stopped `{}` after {} minutes unused; `--keep` or `rusty dev keep <id>` keeps a session",
                    options.project.display(),
                    limit.as_secs() / 60
                );
                return Ok(());
            }
        }
        if let Some(active_child) = child.as_mut() {
            if let Some(status) = active_child.try_wait()? {
                let exited_child = child.take().expect("observed child is present");
                if status.code() == Some(HOST_BIND_FAILURE_EXIT_CODE) {
                    // Restarting cannot free the port; the host printed why.
                    if let Some(session) = session {
                        session.finish("port-unavailable");
                    }
                    diagnostic(
                        "stopped",
                        serde_json::json!({ "reason": "port-unavailable" }),
                    );
                    return Err(
                        "RUSTY_DEV_PORT: the product host could not bind its port (PRODUCT_HOST_BIND above)"
                            .to_owned(),
                    );
                }
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
                            &options,
                            session,
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
                        if let Some(session) = session {
                            session.paused_fault();
                        }
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
        // Rest three times as long as the last look took, so a watch over a
        // network share (a second or more per look) spends at most a quarter
        // of its time walking it; a local tree stays at the poll interval.
        thread::sleep(POLL_INTERVAL.max(last_capture * 3));
        let capture_started = std::time::Instant::now();
        let captured = FileSnapshot::capture(&watches);
        last_capture = capture_started.elapsed();
        let next = match captured {
            Ok(snapshot) => snapshot,
            Err(error) => {
                if !options.project.exists() {
                    // The checkout is gone: nothing can restage or stop this
                    // session through it, so it ends here.
                    if let Some(mut active_child) = child.take() {
                        active_child.shutdown()?;
                    }
                    if let Some(session) = session {
                        session.finish("project-removed");
                    }
                    diagnostic(
                        "stopped",
                        serde_json::json!({ "reason": "project-removed" }),
                    );
                    return Ok(());
                }
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
        if let (false, Some(session)) = (changed.is_empty(), session) {
            session.touch_activity();
        }
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
                &options,
                session,
            )?);
            true
        };
        if started_after_restage && child.is_none() {
            child = Some(SupervisedHost::start(
                &runtime.host,
                &next_staged,
                &persistence_root,
                &options,
                session,
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
    /// `rusty dev --help`, with the `--project` it names, if any.
    DevHelp(Option<PathBuf>),
    Dev(DevOptions),
    /// `rusty dev start`: the options, checked, and the arguments that name them.
    DevStart(DevOptions, Vec<std::ffi::OsString>),
    /// `rusty dev stop --project <p> [--instance <name>]`.
    DevStop(PathBuf, Option<String>),
    /// `rusty dev stop <id|port>`: a session in the machine registry.
    DevStopTarget(String),
    DevList {
        json: bool,
        all: bool,
    },
    DevKeep {
        target: String,
        keep: bool,
    },
    DevPrune,
    DevStatus(PathBuf, Option<String>),
    Build(BuildOptions),
    Install(InstallOptions),
    Update(UpdateOptions),
    Status(StatusOptions),
    PackContent(PackContentOptions),
    AssetCheck(PathBuf),
}

#[derive(Debug)]
struct PackContentOptions {
    directory: PathBuf,
    output: PathBuf,
    compress: bool,
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
    /// `stream` or `window`, for this launch (`RustyEngineProductRenderOutput`).
    output: Option<String>,
    /// `stream`, `device-optional` or `device-required`
    /// (`RustyEngineProductAudioOutput`).
    audio_output: Option<String>,
    /// Chromium switches for the window's UI page, `name[=value]`.
    cef_switches: Vec<String>,
    /// The Chromium executable `--headless` opens.
    chromium: Option<PathBuf>,
    /// Where the host writes its NDJSON diagnostics.
    diagnostics_log: Option<PathBuf>,
    /// Run as the background session `rusty dev start` launched.
    session: bool,
    /// Who or what the session is for, shown by `rusty dev list`.
    label: Option<String>,
    /// Keep the session: idle expiry never stops it.
    keep: bool,
    /// Minutes unused before the session stops itself; 0 never.
    idle_timeout: Option<u64>,
    /// Another concurrent session of the same project, with its own lock.
    instance: Option<String>,
    /// Accept a fixed --port inside the machine's ephemeral range.
    allow_ephemeral_port: bool,
}

#[derive(Debug)]
struct BuildOptions {
    project: PathBuf,
    engine_source: Option<PathBuf>,
    aot: bool,
    /// Release directory for the packed Product.
    pack: Option<PathBuf>,
    /// Store files zstd shrinks enough compressed in the packed Product.
    compress: bool,
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
                Some(_) => CommandName::DevHelp(
                    rest.iter()
                        .position(|value| value == "--project")
                        .and_then(|index| rest.get(index + 1))
                        .map(PathBuf::from)
                        .or_else(|| configured_project().ok().flatten()),
                ),
                None => match rest.first().map(String::as_str) {
                    Some("start") => {
                        let mut arguments: Vec<String> = rest[1..].to_vec();
                        let options = parse_dev(arguments.clone())?;
                        // The session runs the project chosen here.
                        if !arguments.iter().any(|value| value == "--project") {
                            arguments.splice(
                                0..0,
                                [
                                    "--project".to_owned(),
                                    options.project.display().to_string(),
                                ],
                            );
                        }
                        CommandName::DevStart(
                            options,
                            arguments.into_iter().map(Into::into).collect(),
                        )
                    }
                    Some("stop") => match &rest[1..] {
                        [target] if !target.starts_with('-') => {
                            CommandName::DevStopTarget(target.clone())
                        }
                        values => {
                            let (project, instance) = parse_session_project(values, "stop")?;
                            CommandName::DevStop(project, instance)
                        }
                    },
                    Some("list") => match rest[1..]
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .as_slice()
                    {
                        [] => CommandName::DevList {
                            json: false,
                            all: false,
                        },
                        ["--json"] => CommandName::DevList {
                            json: true,
                            all: false,
                        },
                        ["--all"] => CommandName::DevList {
                            json: false,
                            all: true,
                        },
                        ["--json", "--all"] | ["--all", "--json"] => CommandName::DevList {
                            json: true,
                            all: true,
                        },
                        _ => {
                            return Err(unknown_argument(
                                "dev list",
                                &rest[1..].join(" "),
                                dev_usage,
                            ))
                        }
                    },
                    Some("keep") => match &rest[1..] {
                        [target] => CommandName::DevKeep {
                            target: target.clone(),
                            keep: true,
                        },
                        [target, flag] if flag == "--off" => CommandName::DevKeep {
                            target: target.clone(),
                            keep: false,
                        },
                        _ => {
                            return Err(format!(
                            "RUSTY_DEV_ARGUMENT: rusty dev keep needs `<id|port> [--off]`\n\n{}",
                            dev_usage()
                        ))
                        }
                    },
                    Some("prune") if rest.len() == 1 => CommandName::DevPrune,
                    Some("status") => {
                        let (project, instance) = parse_session_project(&rest[1..], "status")?;
                        CommandName::DevStatus(project, instance)
                    }
                    _ => CommandName::Dev(parse_dev(rest)?),
                },
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
            "asset" => match help(asset_usage) {
                Some(help) => return Ok(help),
                None => match rest.as_slice() {
                    [check, path] if check == "check" => {
                        CommandName::AssetCheck(PathBuf::from(path))
                    }
                    _ => {
                        return Err(format!(
                            "RUSTY_ARGUMENT: rusty asset needs `check <file.glb>`\n\n{}",
                            asset_usage()
                        ))
                    }
                },
            },
            "pack-content" => match help(pack_content_usage) {
                Some(help) => return Ok(help),
                None => CommandName::PackContent(parse_pack_content(rest)?),
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
    let mut output = None;
    let mut audio_output = None;
    let mut cef_switches = Vec::new();
    let mut chromium = None;
    let mut diagnostics_log = None;
    let mut session = false;
    let mut label = None;
    let mut keep = false;
    let mut idle_timeout = None;
    let mut instance = None;
    let mut allow_ephemeral_port = false;
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
            "--output" => {
                let value = required_value(&mut values, "--output")?;
                if !matches!(value.as_str(), "stream" | "window") {
                    return Err("RUSTY_DEV_ARGUMENT: --output must be stream or window".to_owned());
                }
                output = Some(value);
            }
            "--audio-output" => {
                let value = required_value(&mut values, "--audio-output")?;
                if !matches!(
                    value.as_str(),
                    "stream" | "device-optional" | "device-required"
                ) {
                    return Err(
                        "RUSTY_DEV_ARGUMENT: --audio-output must be stream, device-optional or device-required"
                            .to_owned(),
                    );
                }
                audio_output = Some(value);
            }
            "--cef-switch" => cef_switches.push(required_value(&mut values, "--cef-switch")?),
            "--chromium" => {
                chromium = Some(PathBuf::from(required_value(&mut values, "--chromium")?))
            }
            "--diagnostics-log" => {
                diagnostics_log = Some(PathBuf::from(required_value(
                    &mut values,
                    "--diagnostics-log",
                )?))
            }
            "--session" => session = true,
            "--label" => {
                let value = required_value(&mut values, "--label")?;
                if value.is_empty() || value.len() > 80 || value.chars().any(char::is_control) {
                    return Err(
                        "RUSTY_DEV_ARGUMENT: --label must be 1 to 80 printable characters"
                            .to_owned(),
                    );
                }
                label = Some(value);
            }
            "--keep" => keep = true,
            "--instance" => {
                instance = Some(instance_name(required_value(&mut values, "--instance")?)?)
            }
            "--allow-ephemeral-port" => allow_ephemeral_port = true,
            "--idle-timeout" => {
                idle_timeout = Some(
                    required_value(&mut values, "--idle-timeout")?
                        .parse()
                        .map_err(|_| "RUSTY_DEV_ARGUMENT: --idle-timeout must be whole minutes")?,
                )
            }
            _ => return Err(unknown_argument("dev", &value, dev_usage)),
        }
    }
    if runtime.is_some() && engine_source.is_some() {
        return Err(
            "RUSTY_DEV_ARGUMENT: --runtime and --engine-source are mutually exclusive".to_owned(),
        );
    }
    if chromium.is_some() && !headless {
        return Err("RUSTY_DEV_ARGUMENT: --chromium selects the --headless browser".to_owned());
    }
    let project = selected_project(project, "rusty dev")?;
    Ok(DevOptions {
        project,
        runtime,
        engine_source,
        bind_host,
        port,
        live_debug,
        debugger,
        headless,
        output,
        audio_output,
        cef_switches,
        chromium,
        diagnostics_log,
        session,
        label,
        keep,
        idle_timeout,
        instance,
        allow_ephemeral_port,
    })
}

/// The explicit `--project`, else the default the repository around the
/// current directory names with `<RustyEngineProject>`.
fn selected_project(project: Option<PathBuf>, command: &str) -> Result<PathBuf, String> {
    if let Some(project) = project {
        return Ok(project);
    }
    configured_project()?.ok_or_else(|| {
        format!(
            "RUSTY_ARGUMENT: {command} needs --project <product.csproj>, or a default named by \
             <{element}>path/to/Product.csproj</{element}> in the repository's {file}",
            element = pair::PROJECT_ELEMENT,
            file = pair::PIN_FILE,
        )
    })
}

fn configured_project() -> Result<Option<PathBuf>, String> {
    let directory =
        env::current_dir().map_err(|error| format!("RUSTY_PATH: no current directory: {error}"))?;
    pair::default_project(&directory)
}

fn parse_session_project(
    values: &[String],
    command: &str,
) -> Result<(PathBuf, Option<String>), String> {
    let refused = || {
        format!(
            "RUSTY_DEV_ARGUMENT: rusty dev {command} needs `[--project <ordinary-product.csproj>] [--instance <name>]`\n\n{}",
            dev_usage()
        )
    };
    let mut project = None;
    let mut instance = None;
    let mut values = values.iter().cloned();
    while let Some(value) = values.next() {
        match value.as_str() {
            "--project" => project = Some(PathBuf::from(values.next().ok_or_else(refused)?)),
            "--instance" => instance = Some(instance_name(values.next().ok_or_else(refused)?)?),
            _ => return Err(refused()),
        }
    }
    Ok((
        selected_project(project, &format!("rusty dev {command}"))?,
        instance,
    ))
}

/// An instance name keys a lock directory: letters, digits, `.`, `_`, `-`.
fn instance_name(value: String) -> Result<String, String> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        });
    if valid {
        Ok(value)
    } else {
        Err(
            "RUSTY_DEV_ARGUMENT: --instance must be 1 to 64 letters, digits, `.`, `_` or `-`"
                .to_owned(),
        )
    }
}

fn parse_build(values: Vec<String>) -> Result<BuildOptions, String> {
    let mut values = values.into_iter();
    let mut project = None;
    let mut engine_source = None;
    let mut aot = false;
    let mut pack = None;
    let mut compress = false;
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
            "--pack" => pack = Some(PathBuf::from(required_value(&mut values, "--pack")?)),
            "--compress" => compress = true,
            _ => return Err(unknown_argument("build", &value, build_usage)),
        }
    }
    if compress && pack.is_none() {
        return Err("RUSTY_ARGUMENT: --compress applies to --pack".to_owned());
    }
    Ok(BuildOptions {
        project: selected_project(project, "rusty build")?,
        engine_source,
        aot,
        pack,
        compress,
    })
}

fn parse_pack_content(values: Vec<String>) -> Result<PackContentOptions, String> {
    let mut values = values.into_iter();
    let mut directory = None;
    let mut output = None;
    let mut compress = false;
    while let Some(value) = values.next() {
        match value.as_str() {
            "--output" => output = Some(PathBuf::from(required_value(&mut values, "--output")?)),
            "--compress" => compress = true,
            _ if !value.starts_with('-') && directory.is_none() => {
                directory = Some(PathBuf::from(value))
            }
            _ => return Err(unknown_argument("pack-content", &value, pack_content_usage)),
        }
    }
    Ok(PackContentOptions {
        directory: directory
            .ok_or("RUSTY_ARGUMENT: rusty pack-content needs a content directory")?,
        output: output.ok_or("RUSTY_ARGUMENT: rusty pack-content needs --output <file>")?,
        compress,
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
  status        show the pinned Engine pair, whether it is installed, paths, dev sessions and
                missing prerequisites
  install       install the pinned SDK/runtime pair into the shared cache (once; later use works
                offline); --archive <pair.tar.gz> installs one you already have
  update        move the pin to a newer published pair, install it, and list what changed
  build         restore, build and stage the product; --aot also publishes NativeAOT;
                --pack <release-dir> [--compress] writes a release product.rpak
  dev           build and run the product on its pinned runtime, rebuilding on source changes
  dev start     the same in the background, one per project; prints where it serves
  dev status    whether the project's background session runs
  dev stop      stop a session: the project's (--project) or any one by `<id|port>`
  dev list      every dev session on this machine, with its id, URL, idle time and label;
                --all adds those that ended for a reason (such as idle expiry) in the last day
  dev keep      keep a session from stopping when idle (`<id|port>`, `--off` to undo)
  dev prune     clear the records of sessions that ended without cleaning up
  pack-content  pack one content directory into a container a product opens at run time
  asset check   check a GLB against the Engine's admission, as JSON, without changing it
                (exit 0 admitted, 1 refused, 2 unreadable)

Run `rusty <command> --help` for a command's options.

Everyday use, from the product repository. Its Directory.Build.props names the default project
with <{project}>src/Game/Game.csproj</{project}>, so --project can be left out:
  rusty status
  rusty install
  rusty dev                           foreground; Ctrl+C stops it
  rusty dev start --label <who>       background; prints its id and URL
  rusty dev list
  rusty dev stop <id|port>
  rusty update --check
  rusty update
  rusty build --aot

Dev sessions: every `rusty dev` is listed by `rusty dev list`, one per project unless
`--instance <name>` runs another. Stop one with `rusty dev stop`, never by killing rusty or
rusty-product-host processes: on a shared machine they belong to other sessions. A session unused
for {idle} minutes (no input, control, live-debug command, page attaching or restage; a page that
only watches does not count) stops itself. `--keep` or `rusty dev keep <id>` keeps one;
`--idle-timeout <minutes>` or devIdleMinutes in {config} sets the limit (0 never). Leave out
--port: a free port is chosen and printed. A fixed port inside the machine's ephemeral range
(often 32768-60999) is refused, because outgoing connections borrow those ports.

Development state is disposable. Everything under a product's .runtime/ (dev logs, session
records, saves and other persistence from test runs) may be deleted, reset or made unreadable by
a pair update, a restage or a clean, and is never migrated between versions. Don't spend effort
preserving or migrating it. If one save or file must be kept, for a reproduction or a test,
copy it out of .runtime/ to a place the product owns (a committed fixture such as
tests/fixtures/, or your evidence location), note what it shows and which pair made it, and load
it from there.

The pin is the one <{pin}> element in the product's {pin_file}.
Nothing moves it except `rusty update`. Installed pairs and the dev session list live in {cache}
(under XDG_CACHE_HOME when set). Its {config} may set {{\"releases\": \"<url>\"}} (a release mirror),
{{\"localOutput\": \"<dir>\"}} (keep build output and .runtime state there instead of in each checkout;
`rusty status` shows where) and {{\"devIdleMinutes\": <minutes>}}.

Get or refresh this command:
  {bootstrap}",
        project = pair::PROJECT_ELEMENT,
        idle = DEFAULT_IDLE_MINUTES,
        pin = pair::PIN_ELEMENT,
        pin_file = pair::PIN_FILE,
        cache = pair::cache_root().map_or_else(|error| error, |root| root.display().to_string()),
        config = pair::CONFIG_FILE,
        bootstrap = BOOTSTRAP,
    )
}

fn dev_usage() -> String {
    format!("usage: rusty dev [--project <ordinary-product.csproj>] [--port <u16>] [--bind-host <IPv4>] [--live-debug] [--debugger]
                 [--headless [--chromium <executable>]] [--output <stream|window>]
                 [--audio-output <stream|device-optional|device-required>] [--cef-switch <name[=value]>]...
                 [--diagnostics-log <file>] [--label <text>] [--keep] [--idle-timeout <minutes>]
                 [--instance <name>] [--allow-ephemeral-port]
                 [--runtime <runtime-pack> | --engine-source <rusty-engine-source>]
       rusty dev start [the same options]
       rusty dev stop|status [--project <ordinary-product.csproj>] [--instance <name>]
       rusty dev stop <id|port>
       rusty dev list [--json] [--all]
       rusty dev keep <id|port> [--off]
       rusty dev prune

Without --project, the product is the one the nearest Directory.Build.props at or above the current
directory names, relative to itself, with <RustyEngineProject>src/Game/Game.csproj</RustyEngineProject>.

Builds and stages the product through its SDK, starts it on CoreCLR, and restages when declared
C#, UI or content inputs change. UI and content-bundle edits reload into the running product; other
edits replace the runtime.

One session runs per project, foreground or background: a second is refused while one runs.
`--instance <name>` runs another beside it under its own name (crew playtest runs one host per
playtest session this way); stop and status take the same --instance.

`rusty dev start` runs the same session in the background: it returns once the
product serves and prints {{id, url, port, pid, runtimeInstanceId, persistenceRoot, log}} as JSON, or exits
nonzero with the log's tail if staging or startup failed. `rusty dev stop` ends that project's session
and disposes the product as Ctrl+C would; `rusty dev status` reports it. The session's log lives in
the repository's .runtime/dev/, one directory per project path, or under this machine's
localOutput when its config.json names one (`rusty status` shows where).

Every `rusty dev`, foreground or background, is in this machine's session list (`rusty dev list`;
`--json` for the records). `rusty dev stop <id|port>` stops any of them gracefully, then signals
only the processes its record names: `rusty dev`, its host supervisor, and the process groups of
its runtime and headless browser. Never stop a host by killing processes by name: on a shared
machine they belong to other sessions. `--label` says who or what a session is for. A session that
ends for a reason a caller must see (idle-expired, port-unavailable, project-removed) keeps its
record for a day, listed by `rusty dev list --all`; a session that stopped or crashed leaves none.

A session unused for {idle} minutes stops itself. Use is input, a control or lifecycle call, a live-debug
command, a page attaching or a restage; a page or browser that only watches is not use. `--keep` or
`rusty dev keep <id|port>` keeps a session (an owner's long-running host) until `--off`;
`--idle-timeout <minutes>` or {{\"devIdleMinutes\": <minutes>}} in config.json sets the limit, 0 for
never. Window output never expires: the window is someone's.

Leave out --port and a free port is chosen and printed. A fixed port inside this machine's
ephemeral range (often 32768-60999) is refused before anything starts: outgoing connections borrow
those ports, so one can be taken with nothing listening. Choose one below the range, or pass
--allow-ephemeral-port to keep a port deliberately. A port that is taken stops the host at once
with PRODUCT_HOST_BIND.

Everything under .runtime/ is disposable test state (logs, records, saves) that a pair update,
restage or clean may delete or invalidate, and nothing migrates it. Copy a save that must be kept
to a place the product owns and load it from there.

The runtime is the pair pinned in the product's Directory.Build.props, installed by `rusty install`.
`rusty dev` runs that pair's own copy of this command, so the supervisor always matches its host;
in a pinned product, `rusty dev --help` shows that copy's help, whose options are the ones that apply.
A product whose project sets RustyEngineProductRenderOutput=window (or a run with --output window)
opens in a native window; the first such run downloads the pair's desktop runtime pack (Chromium's
runtime for the UI) into the cache beside the pair.

  --port, --bind-host  where the browser host listens
  --live-debug         enable the live-debug command surface
  --debugger           no runtime startup deadline, for managed breakpoints
  --headless           run unattended: a headless Chromium page keeps the world drawing and the UI mounted
  --chromium           the Chromium executable --headless opens (else one on PATH)
  --output             stream (the default) or window, for this launch
  --audio-output       stream plays in the watching pages (the default with stream output); device-required
                       fails the load without an audio device; device-optional runs silent
  --cef-switch         a Chromium switch for the window's UI page, e.g. remote-debugging-port=9333
  --diagnostics-log    write the host's NDJSON diagnostics to this file
  --label              who or what the session is for, shown by `rusty dev list`
  --keep               never stop this session for being idle
  --idle-timeout       minutes unused before the session stops itself (0 never)
  --instance           run another session of a project that already has one, under this name
  --allow-ephemeral-port  accept a fixed --port inside the machine's ephemeral range
  --runtime            Engine contributors: use this runtime pack instead of the pin
  --engine-source      Engine contributors: build the SDK and runtime from this checkout

This command never invokes Cargo and never searches for an adjacent Engine checkout.

Examples:
  rusty dev --output window
  rusty dev --project src/Game/Game.csproj --live-debug --headless
  rusty dev start --project src/Game/Game.csproj --live-debug --label agent:reviewer
  rusty dev list
  rusty dev stop 30302
  rusty dev start --keep --bind-host 0.0.0.0 --port 30400", idle = DEFAULT_IDLE_MINUTES)
}

fn build_usage() -> String {
    "usage: rusty build [--project <product.csproj>] [--aot] [--pack <release-dir> [--compress]] [--engine-source <rusty-engine-source>]

Restores against the pinned SDK in the shared cache, builds, and stages the CoreCLR product bundle
(the SDK target StageRustyEngineCoreClrProduct). --aot runs VerifyRustyEngineAot, which also
publishes the NativeAOT product. Compiler output and dotnet's exit code are passed through.
Without --project, it builds the repository's <RustyEngineProject> default, as `rusty dev` does.

--pack then writes the staged Product as a release: <release-dir>/product.rpak holds product.json,
the UI and the content; the CoreCLR assemblies or NativeAOT module are copied loose beside it.
Run it with `rusty-product-host --product <release-dir>/product.rpak --loader <coreclr|nativeaot>`.
--compress stores each file zstd shrinks by at least a tenth compressed (text and JSON, not media);
the rest stay raw.

Plain `dotnet build`, `dotnet test` and `dotnet run` resolve the same SDK: the product's
Directory.Build.props declares the pinned pair's feed (`rusty status` checks it).

Examples:
  rusty build --project src/Game/Game.csproj
  rusty build --project src/Game/Game.csproj --aot
  rusty build --project src/Game/Game.csproj --pack release
  rusty build --project src/Game/Game.csproj --pack release --compress"
        .to_owned()
}

fn pack_content_usage() -> String {
    "usage: rusty pack-content <directory> --output <file> [--compress]

Packs every file under <directory>, at its directory-relative path, into one content container: the
format `rusty build --pack` writes, with each file's length and SHA-256, and no product manifest,
UI or code. A running product opens it with `Content.OpenContainer(path)` as an ordinary content
bundle. --compress stores each file zstd shrinks by at least a tenth compressed. The file appears
at <file> by rename once complete; the output must lie outside <directory>.

Example:
  rusty pack-content modules/srd-ruleset --output library/srd-ruleset.rpak --compress"
        .to_owned()
}

fn asset_usage() -> String {
    "usage: rusty asset check <file.glb>

Runs the admission a running product applies to a standalone GLB (Content.AdmitReference, then
Animation.OpenAnimatedMeshFromContent) and prints one JSON object:

  {\"path\": \"<file.glb>\", \"admitted\": true|false,
   \"diagnostics\": [{\"severity\", \"code\", \"locus\", \"message\", \"remedy\"}, ...]}

Codes are the asset importer's (externalResource, unsupportedFeature, invalidContainer, ...); the
locus names the GLB JSON member. The file is read and never written. It exits 0 when admitted, 1
when refused, and 2 when the file cannot be read. The rules are those of the `rusty` that runs:
run the pinned pair's `runtime-pack/bin/rusty` (under the cache `rusty status` shows) to check
against the pair a product pins.

Example:
  rusty asset check exports/knight.glb"
        .to_owned()
}

fn asset_check(path: &Path) -> Result<ExitCode, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("RUSTY_ASSET_READ: `{}`: {error}", path.display());
            return Ok(ExitCode::from(2));
        }
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source.glb");
    let (admitted, diagnostics) = asset::check_glb(file_name, bytes);
    println!("{}", asset::report(path, admitted, &diagnostics));
    Ok(if admitted {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn pack_content(options: &PackContentOptions) -> Result<ExitCode, String> {
    let report =
        product_container::pack_content(&options.directory, &options.output, options.compress)
            .map_err(|error| format!("RUSTY_PACK_CONTENT: {error}"))?;
    println!(
        "Packed {} files ({} bytes) into {}",
        report.entries,
        report.bytes,
        options.output.display()
    );
    Ok(ExitCode::SUCCESS)
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
            absolute(source)?
                .join("target/runtime-pack")
                .join(pair::TARGET)
        } else {
            runtime_beside_current_executable(&options.project)?
        };
        Self::at(root)
    }

    fn at(root: PathBuf) -> Result<Self, String> {
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
        let host = root.join(format!("bin/rusty-product-host{}", env::consts::EXE_SUFFIX));
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
        if self.manifest.target != pair::TARGET {
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
    // Resolves a symlinked dotnet; Windows' verbatim `\\?\` prefix is not a
    // DOTNET_ROOT .NET accepts.
    let resolved = fs::canonicalize(dotnet).ok()?;
    let resolved = match resolved
        .to_str()
        .and_then(|path| path.strip_prefix(r"\\?\"))
    {
        Some(plain) => PathBuf::from(plain),
        None => resolved,
    };
    resolved.parent().map(Path::to_owned)
}

/// The executable `name` (`name.exe` on Windows) in a `PATH` directory.
fn find_on_path(name: &str) -> Option<PathBuf> {
    find_in(&env::var_os("PATH")?, name)
}

fn find_in(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    let file = format!("{name}{}", env::consts::EXE_SUFFIX);
    env::split_paths(path)
        .map(|directory| directory.join(&file))
        .find(|candidate| candidate.is_file())
}

/// The staged manifest's `renderer.output` is `window`: the runtime presents
/// to a native window.
fn window_output(staged: &Path) -> Result<bool, String> {
    let manifest = staged.join("product.json");
    let bytes = fs::read(&manifest).map_err(|error| {
        format!(
            "RUSTY_DEV_STAGE: could not read `{}`: {error}",
            manifest.display()
        )
    })?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "RUSTY_DEV_STAGE: `{}` is invalid JSON: {error}",
            manifest.display()
        )
    })?;
    Ok(manifest["renderer"]["output"] == "window")
}

/// Whether `rusty dev` selects the pinned pair's pack itself: neither
/// --runtime nor --engine-source was given. Delegation to the pair's own
/// `rusty` passes no --runtime either, so every --runtime is the caller's
/// explicit choice.
fn pinned_selection(options: &DevOptions) -> bool {
    options.runtime.is_none() && options.engine_source.is_none()
}

/// Window output runs a desktop pack: the same host with the desktop shell
/// and Chromium's runtime (`lib/cef`). The pinned pair's pack is swapped for
/// the pair's desktop pack, installed on first use; an explicit runtime pack
/// must already be one.
fn window_runtime(
    runtime: RuntimePack,
    staged: &Path,
    pinned_selection: bool,
) -> Result<RuntimePack, String> {
    if !window_output(staged)? {
        return Ok(runtime);
    }
    let Some(pair) = desktop_pair_for(&runtime.root, pinned_selection)? else {
        return Ok(runtime);
    };
    let (root, downloaded) = pair::install_desktop_pack(&pair)?;
    diagnostic(
        "desktop-pack",
        serde_json::json!({ "runtimePack": root, "downloaded": downloaded }),
    );
    let desktop = RuntimePack::at(root)?;
    desktop.verify()?;
    Ok(desktop)
}

/// The pair whose desktop pack replaces `root` in window output, or `None`
/// when `root` already is a desktop pack. Only the pinned pair's selection is
/// swapped.
fn desktop_pair_for(
    root: &Path,
    pinned_selection: bool,
) -> Result<Option<pair::InstalledPair>, String> {
    if root.join("lib/cef").is_dir() {
        return Ok(None);
    }
    let refused = || {
        format!(
            "RUSTY_DEV_WINDOW: window output needs a desktop runtime pack; `{}` has no lib/cef. Pass --runtime <desktop pack>, or omit --runtime to use the pinned pair's",
            root.display()
        )
    };
    if !pinned_selection {
        return Err(refused());
    }
    root.parent()
        .filter(|pair| {
            root.file_name() == Some("runtime-pack".as_ref())
                && pair.join("pair-manifest.json").is_file()
        })
        .and_then(|pair| {
            Some(pair::InstalledPair {
                version: pair.file_name()?.to_str()?.to_owned(),
                root: pair.to_owned(),
            })
        })
        .map(Some)
        .ok_or_else(refused)
}

/// Runs the pinned pair's own `rusty dev`, whose supervisor protocol and
/// staging expectations match that pair's host. Returns when this process is
/// already that command.
#[cfg(unix)]
fn delegate_to_pair_cli(runtime: &Path, options: &DevOptions) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    let pair_cli = runtime.join(format!("bin/rusty{}", env::consts::EXE_SUFFIX));
    if is_current_exe(&pair_cli) {
        return Ok(());
    }
    let error = Command::new(&pair_cli)
        .args(delegated_arguments(options))
        .exec();
    Err(format!(
        "RUSTY_DEV_RUNTIME: could not run the pinned pair's `{}`: {error}; run `rusty install` again if the cache was edited",
        pair_cli.display()
    ))
}

/// The pinned pair's own `rusty`, when the product at `project` (or the
/// current directory) pins an installed pair. Help that cannot find one is
/// this command's own.
fn pinned_pair_cli(project: Option<&Path>) -> Option<PathBuf> {
    let pin = Pin::find(&product_start(project).ok()?).ok()??;
    let pair = pair::installed(&pin.version).ok()??;
    Some(
        pair.runtime_pack()
            .join(format!("bin/rusty{}", env::consts::EXE_SUFFIX)),
    )
}

/// Shows the pinned pair's `rusty dev --help`: that copy runs `rusty dev`, so
/// an older pair's options, not this command's, are the ones that apply.
/// Returns when this process is already that command.
#[cfg(unix)]
fn delegate_help_to_pair_cli(pair_cli: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;

    if is_current_exe(pair_cli) {
        return Ok(());
    }
    let error = Command::new(pair_cli).args(["dev", "--help"]).exec();
    Err(format!(
        "RUSTY_DEV_RUNTIME: could not run the pinned pair's `{}`: {error}; run `rusty install` again if the cache was edited",
        pair_cli.display()
    ))
}

#[cfg(not(unix))]
fn delegate_help_to_pair_cli(pair_cli: &Path) -> Result<(), String> {
    if is_current_exe(pair_cli) {
        return Ok(());
    }
    let status = Command::new(pair_cli)
        .args(["dev", "--help"])
        .status()
        .map_err(|error| {
            format!(
                "RUSTY_DEV_RUNTIME: could not run the pinned pair's `{}`: {error}; run `rusty install` again if the cache was edited",
                pair_cli.display()
            )
        })?;
    std::process::exit(status.code().unwrap_or(1));
}

fn is_current_exe(path: &Path) -> bool {
    let current = env::current_exe().and_then(fs::canonicalize).ok();
    current.is_some() && current == fs::canonicalize(path).ok()
}

/// The pinned pair's own `rusty dev` arguments: the caller's options without
/// --runtime, so that run resolves the pin itself (a pack's own `rusty` uses
/// its pack) and still counts as the pinned selection.
fn delegated_arguments(options: &DevOptions) -> Vec<std::ffi::OsString> {
    let mut arguments: Vec<std::ffi::OsString> = vec![
        "dev".into(),
        "--project".into(),
        options.project.clone().into(),
    ];
    let mut push = |flag: &str, value: std::ffi::OsString| {
        arguments.push(flag.into());
        arguments.push(value);
    };
    if let Some(bind_host) = &options.bind_host {
        push("--bind-host", bind_host.into());
    }
    if let Some(port) = options.port {
        push("--port", port.to_string().into());
    }
    if let Some(output) = &options.output {
        push("--output", output.into());
    }
    if let Some(audio_output) = &options.audio_output {
        push("--audio-output", audio_output.into());
    }
    for switch in &options.cef_switches {
        push("--cef-switch", switch.into());
    }
    if let Some(chromium) = &options.chromium {
        push("--chromium", chromium.clone().into());
    }
    if let Some(log) = &options.diagnostics_log {
        push("--diagnostics-log", log.clone().into());
    }
    for (enabled, flag) in [
        (options.live_debug, "--live-debug"),
        (options.debugger, "--debugger"),
        (options.headless, "--headless"),
        (options.session, "--session"),
    ] {
        if enabled {
            arguments.push(flag.into());
        }
    }
    arguments
}

/// Windows has no exec: run the pinned pair's `rusty dev` in this console,
/// which delivers Ctrl+C to both, and exit with its code.
#[cfg(not(unix))]
fn delegate_to_pair_cli(runtime: &Path, options: &DevOptions) -> Result<(), String> {
    let pair_cli = runtime.join(format!("bin/rusty{}", env::consts::EXE_SUFFIX));
    if is_current_exe(&pair_cli) {
        return Ok(());
    }
    let status = Command::new(&pair_cli)
        .args(delegated_arguments(options))
        .status()
        .map_err(|error| {
            format!(
                "RUSTY_DEV_RUNTIME: could not run the pinned pair's `{}`: {error}; run `rusty install` again if the cache was edited",
                pair_cli.display()
            )
        })?;
    std::process::exit(status.code().unwrap_or(1));
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
    arguments.extend(DevelopmentRoots::of(&project)?.msbuild_properties()?);
    let result_file =
        env::temp_dir().join(format!("rusty-build-stage-{}.json", std::process::id()));
    if options.pack.is_some() {
        arguments.push(format!("-getProperty:{STAGED_PRODUCT_PROPERTY}"));
        arguments.push(format!(
            "-getResultOutputFile:{}",
            result_file
                .to_str()
                .ok_or("RUSTY_BUILD: temporary result path must be UTF-8")?
        ));
    }
    let status = Command::new("dotnet")
        .args(&arguments)
        .status()
        .map_err(|error| format!("RUSTY_PREREQUISITE: could not start dotnet: {error}; install the .NET {REQUIRED_DOTNET_MAJOR} SDK"))?;
    if status.success() {
        if let Some(release) = &options.pack {
            // With one -getProperty, MSBuild writes the bare value.
            let result = fs::read_to_string(&result_file);
            let _ = fs::remove_file(&result_file);
            let result = result.map_err(|error| {
                format!("RUSTY_BUILD_PACK: staging produced no property result: {error}")
            })?;
            let staged = Some(result.trim())
                .filter(|value| !value.is_empty())
                .ok_or("RUSTY_BUILD_PACK: staging reported no product directory")?;
            let release = absolute(release)?;
            let report =
                product_container::pack_product(Path::new(staged), &release, options.compress)
                    .map_err(|error| format!("RUSTY_BUILD_PACK: {error}"))?;
            println!(
                "Packed {} files ({} bytes) into {}; {} native files beside it",
                report.packed_files,
                report.container_bytes,
                report.container.display(),
                report.loose_files.len()
            );
        }
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
                        println!(
                            "desktop pack   not installed (fetched on the first window-output run)"
                        );
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
    match pair::default_project(&start) {
        Ok(Some(project)) => println!("project        {}", project.display()),
        Ok(None) => println!("project        none named; pass --project"),
        Err(error) => problems.push(error),
    }
    // Any file in `start` finds the same checkout as a project there would.
    let roots = DevelopmentRoots::of(&start.join(pair::PIN_FILE))?;
    println!("dev state      {}", roots.runtime.display());
    if let Some((registry, running)) = session::registry_summary() {
        println!(
            "dev sessions   {running} running on this machine; `rusty dev list` ({})",
            registry.display()
        );
    }
    match &roots.artifacts {
        Some(artifacts) => println!("build output   {}", artifacts.display()),
        None => println!("build output   bin/ and obj/ beside each project"),
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
    properties.extend(DevelopmentRoots::of(&options.project)?.msbuild_properties()?);
    if let Some(bind_host) = &options.bind_host {
        properties.push(format!("-p:RustyEngineProductBindHost={bind_host}"));
    }
    if let Some(port) = options.port {
        properties.push(format!("-p:RustyEngineProductPort={port}"));
    }
    if options.live_debug {
        properties.push("-p:RustyEngineProductLiveDebug=true".to_owned());
    }
    if let Some(output) = &options.output {
        properties.push(format!("-p:RustyEngineProductRenderOutput={output}"));
    }
    if let Some(audio_output) = &options.audio_output {
        properties.push(format!("-p:RustyEngineProductAudioOutput={audio_output}"));
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

/// Where a product's development state lives. Ordinarily the runtime state
/// is `.runtime/` in the product's repository and MSBuild writes its usual
/// `bin/` and `obj/`. With `localOutput` set in the CLI's `config.json`, this
/// machine keeps both under `<localOutput>/<checkout>/` instead, so machines
/// sharing one checkout (a network drive) never write each other's files.
struct DevelopmentRoots {
    /// The repository holding the project: the `.git` ancestor, or the
    /// project's own directory.
    checkout: PathBuf,
    /// Session records, logs and persistence.
    runtime: PathBuf,
    /// The MSBuild `ArtifactsPath` for this machine's build output.
    artifacts: Option<PathBuf>,
}

impl DevelopmentRoots {
    fn of(project: &Path) -> Result<Self, String> {
        Self::with_local_output(project, pair::local_output().as_deref())
    }

    fn with_local_output(project: &Path, local_output: Option<&Path>) -> Result<Self, String> {
        let project = absolute(project)?;
        let project_directory = project.parent().ok_or_else(|| {
            format!(
                "RUSTY_DEV_PROJECT: ordinary product project `{}` has no containing directory",
                project.display()
            )
        })?;
        // A loose project still gets a stable root beside its project file.
        let checkout = project_directory
            .ancestors()
            .find(|candidate| candidate.join(".git").exists())
            .unwrap_or(project_directory)
            .to_path_buf();
        Ok(match local_output {
            None => Self {
                runtime: checkout.join(".runtime"),
                artifacts: None,
                checkout,
            },
            Some(local_output) => {
                let local = absolute(local_output)?.join(checkout_key(&checkout));
                Self {
                    runtime: local.join("runtime"),
                    artifacts: Some(local.join("artifacts")),
                    checkout,
                }
            }
        })
    }

    fn persistence(&self) -> PathBuf {
        self.runtime.join("persistence")
    }

    /// `-p:ArtifactsPath=…` when this machine keeps build output locally.
    fn msbuild_properties(&self) -> Result<Vec<String>, String> {
        let Some(artifacts) = &self.artifacts else {
            return Ok(Vec::new());
        };
        let artifacts = artifacts
            .to_str()
            .ok_or("RUSTY_LOCAL_OUTPUT: localOutput must be a UTF-8 path")?;
        Ok(vec![format!("-p:ArtifactsPath={artifacts}")])
    }
}

/// A readable directory name for a checkout path: `P:\dev\game` and
/// `/home/me/dev/game` become `P_dev_game` and `home_me_dev_game`.
fn checkout_key(checkout: &Path) -> String {
    let mut key = String::new();
    for character in checkout.to_string_lossy().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '.') {
            key.push(character);
        } else if !key.is_empty() && !key.ends_with('_') {
            key.push('_');
        }
    }
    key.trim_end_matches('_').to_owned()
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
        options: &DevOptions,
        session: Option<&Arc<Session>>,
    ) -> Result<Self, String> {
        let runtime_instance_id = next_supervised_runtime_instance_id()?;
        let mut arguments =
            supervised_host_arguments(product, persistence_root, runtime_instance_id, options)?;
        if let Some(session) = session {
            arguments.push("--activity-file".to_owned());
            arguments.push(
                session
                    .activity_file()
                    .to_str()
                    .ok_or("RUSTY_DEV_SESSION: the session registry path must be UTF-8")?
                    .to_owned(),
            );
        }
        let mut command = Command::new(host);
        command.args(&arguments).stdin(Stdio::piped());
        // The session learns where the host serves from its output.
        if session.is_some() {
            command.stdout(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| {
            format!(
                "RUSTY_DEV_CHILD_START: could not launch `{}`: {error}",
                host.display()
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or("RUSTY_DEV_CHILD_START: supervised child stdin was unavailable")?;
        if let (Some(session), Some(output)) = (session, child.stdout.take()) {
            session.host_started(child.id());
            session.forward_host_output(output, runtime_instance_id);
        }
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
                // Ask the host to stop its runtime and browser itself before
                // forcing it, so neither outlives it holding the port.
                terminate(&mut self.child, Duration::from_secs(5));
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
    options: &DevOptions,
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
    if options.debugger {
        arguments.push("--debugger".to_owned());
    }
    if options.headless {
        arguments.push("--headless".to_owned());
    }
    let path = |path: &Path, what: &str| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("RUSTY_DEV_ARGUMENT: the {what} path must be UTF-8"))
    };
    if let Some(chromium) = &options.chromium {
        arguments.extend(["--chromium".to_owned(), path(chromium, "--chromium")?]);
    }
    for switch in &options.cef_switches {
        arguments.extend(["--cef-switch".to_owned(), switch.clone()]);
    }
    if let Some(log) = &options.diagnostics_log {
        let log = absolute(log)?;
        arguments.extend([
            "--diagnostics-log".to_owned(),
            path(&log, "--diagnostics-log")?,
        ]);
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
            // The listing's own metadata: Windows has it from the directory
            // read, so a share is not asked again for every file.
            match entry.metadata() {
                Ok(metadata) if metadata.is_file() => {
                    files.insert(
                        child,
                        FileStamp {
                            modified: metadata.modified().ok(),
                            bytes: metadata.len(),
                        },
                    );
                }
                _ => capture_path(&child, files)?,
            }
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

/// A fixed `--port` inside the machine's ephemeral range can be taken by an
/// outgoing connection's local port with nothing listening, so it is refused
/// unless `--allow-ephemeral-port` says it was chosen on purpose.
fn check_port(options: &DevOptions) -> Result<(), String> {
    match (options.port, ephemeral_port_range()) {
        (Some(port), Some((low, high)))
            if port != 0 && (low..=high).contains(&port) && !options.allow_ephemeral_port =>
        {
            Err(format!(
                "RUSTY_DEV_PORT: --port {port} is in this machine's ephemeral range {low}-{high}, which outgoing connections borrow, so it can be taken with nothing listening. Leave out --port (a free one is chosen and printed), choose one below {low} (30300-30450 is den-serve's range), or pass --allow-ephemeral-port to keep this one"
            ))
        }
        _ => Ok(()),
    }
}

fn ephemeral_port_range() -> Option<(u16, u16)> {
    let range = fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range").ok()?;
    let mut bounds = range.split_whitespace().map(str::parse::<u16>);
    Some((bounds.next()?.ok()?, bounds.next()?.ok()?))
}

/// Minutes a dev session may go unused before it stops itself, unless
/// `config.json` (`devIdleMinutes`) or `--idle-timeout` say otherwise.
const DEFAULT_IDLE_MINUTES: u64 = 30;

/// How long the session may go unused, or why it never expires: a desktop
/// window (someone's screen; its input bypasses the host's routes) or a limit
/// of 0. Keep is separate: `rusty dev keep` sets and clears it while the
/// session runs, so the loop reads it each time.
fn idle_limit(options: &DevOptions, window: bool) -> Result<Duration, &'static str> {
    if window {
        return Err("window-output");
    }
    match options
        .idle_timeout
        .or_else(pair::dev_idle_minutes)
        .unwrap_or(DEFAULT_IDLE_MINUTES)
    {
        0 => Err("disabled"),
        minutes => Ok(Duration::from_secs(minutes.saturating_mul(60))),
    }
}

/// SIGTERM, then SIGKILL after `grace`; reaps `child` either way.
fn terminate(child: &mut Child, grace: Duration) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
    #[cfg(not(unix))]
    let _ = grace;
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    #[test]
    fn path_lookup_finds_the_platform_executable() {
        let directory = std::env::temp_dir().join(format!("rusty-path-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join(format!("dotnet{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&file, b"").unwrap();
        let path = std::env::join_paths([&directory]).unwrap();
        assert_eq!(super::find_in(&path, "dotnet"), Some(file));
        assert_eq!(super::find_in(&path, "curl"), None);
        std::fs::remove_dir_all(directory).unwrap();
    }

    use super::*;

    #[test]
    fn the_top_level_help_names_every_command() {
        // The commands are read from `Arguments::parse`'s own dispatch arms,
        // so a new command or dev subcommand cannot skip this check.
        let source = include_str!("main.rs");
        let start = source
            .find("    fn parse(values: impl IntoIterator<Item = String>)")
            .unwrap();
        let end = start + source[start..].find("    const fn help(").unwrap();
        let mut commands = Vec::new();
        for line in source[start..end].lines().map(str::trim_start) {
            let (prefix, arm) = if let Some(arm) = line.strip_prefix("Some(\"") {
                ("dev ", arm)
            } else if let Some(arm) = line.strip_prefix('"') {
                ("", arm)
            } else {
                continue;
            };
            if let Some((name, rest)) = arm.split_once('"') {
                let rest = rest.trim_start_matches(')').trim_start();
                if (rest.starts_with("=>") || rest.starts_with("if ")) && name != "help" {
                    commands.push(format!("{prefix}{name}"));
                }
            }
        }
        for expected in [
            "dev",
            "dev start",
            "dev list",
            "dev prune",
            "asset",
            "build",
        ] {
            assert!(
                commands.iter().any(|command| command == expected),
                "{commands:?}"
            );
        }
        let help = usage();
        for command in &commands {
            assert!(
                help.lines()
                    .any(|line| line.trim_start().starts_with(command.as_str())),
                "`rusty {command}` is dispatched but missing from `rusty --help`"
            );
        }
    }

    #[test]
    fn a_fixed_port_in_the_ephemeral_range_is_refused_unless_allowed() {
        let Some((low, high)) = ephemeral_port_range() else {
            return;
        };
        let options = |arguments: &[String]| {
            let mut values = vec!["--project".to_owned(), "P.csproj".to_owned()];
            values.extend(arguments.iter().cloned());
            parse_dev(values).unwrap()
        };
        let port = |port: u16| vec!["--port".to_owned(), port.to_string()];
        let refused = check_port(&options(&port(high))).unwrap_err();
        assert!(refused.contains(&format!("{low}-{high}")), "{refused}");
        assert!(check_port(&options(&port(low))).is_err());
        let mut allowed = port(low);
        allowed.push("--allow-ephemeral-port".to_owned());
        assert!(check_port(&options(&allowed)).is_ok());
        assert!(check_port(&options(&[])).is_ok());
        assert!(check_port(&options(&port(0))).is_ok());
        if low > 1 {
            assert!(check_port(&options(&port(low - 1))).is_ok());
        }
    }

    #[test]
    fn a_session_expires_unless_kept_windowed_or_disabled() {
        let options = |arguments: &[&str]| {
            let mut values = vec!["--project".to_owned(), "P.csproj".to_owned()];
            values.extend(arguments.iter().map(|value| (*value).to_owned()));
            parse_dev(values).unwrap()
        };
        assert_eq!(
            idle_limit(&options(&["--idle-timeout", "5"]), false),
            Ok(Duration::from_secs(300))
        );
        // Keep does not remove the limit: clearing it re-enables expiry.
        assert_eq!(
            idle_limit(&options(&["--keep", "--idle-timeout", "5"]), false),
            Ok(Duration::from_secs(300))
        );
        assert_eq!(idle_limit(&options(&[]), true), Err("window-output"));
        assert_eq!(
            idle_limit(&options(&["--idle-timeout", "0"]), false),
            Err("disabled")
        );
        assert!(parse_dev(vec!["--idle-timeout".to_owned(), "soon".to_owned()]).is_err());
        assert!(parse_dev(vec!["--label".to_owned(), String::new()]).is_err());
    }

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
            &options,
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
            &options,
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
            output: None,
            audio_output: None,
            cef_switches: Vec::new(),
            chromium: None,
            diagnostics_log: None,
            session: false,
            label: None,
            keep: false,
            idle_timeout: None,
            instance: None,
            allow_ephemeral_port: false,
        };

        let properties = stage_properties(&options).expect("source properties");
        assert!(properties.contains(&"-p:RustyEngineUseSourceDevelopment=true".to_owned()));
        assert!(
            properties.contains(&"-p:RustyEngineSourceDevelopmentPath=/engine-source".to_owned())
        );
    }

    #[test]
    fn development_runtime_roots_are_repository_local_and_absolute() {
        let persistence_root =
            DevelopmentRoots::with_local_output(Path::new("src/Product.csproj"), None)
                .expect("development roots")
                .persistence();
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
        let persistence_root = DevelopmentRoots::with_local_output(
            Path::new("/workspace/Product/Product.csproj"),
            None,
        )
        .expect("development roots")
        .persistence();
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
    fn local_output_keeps_each_checkouts_state_and_build_output_apart() {
        let local = Path::new("/local/rusty");
        let roots = |project: &str| {
            DevelopmentRoots::with_local_output(Path::new(project), Some(local)).unwrap()
        };
        let game = roots("/mnt/share/dev/game/Game.csproj");
        assert_eq!(game.checkout, PathBuf::from("/mnt/share/dev/game"));
        assert_eq!(
            game.persistence(),
            local.join("mnt_share_dev_game/runtime/persistence")
        );
        assert_eq!(
            game.msbuild_properties().unwrap(),
            ["-p:ArtifactsPath=/local/rusty/mnt_share_dev_game/artifacts"]
        );
        assert_ne!(
            roots("/mnt/share/dev/other/Game.csproj").runtime,
            game.runtime
        );
        assert_eq!(
            checkout_key(Path::new(r"\\?\P:\dev\rusty-dagger")),
            "P_dev_rusty-dagger"
        );
        assert!(
            DevelopmentRoots::with_local_output(Path::new("/p/Game.csproj"), None)
                .unwrap()
                .msbuild_properties()
                .unwrap()
                .is_empty()
        );
    }

    fn dev_options(extra: &[&str]) -> DevOptions {
        let arguments = Arguments::parse(
            ["dev", "--project", "Product.csproj"]
                .iter()
                .chain(extra)
                .map(|value| (*value).to_owned()),
        )
        .expect("dev options");
        let CommandName::Dev(options) = arguments.command else {
            panic!("dev command");
        };
        options
    }

    #[test]
    fn output_options_stage_and_launch_options_reach_the_host() {
        let options = dev_options(&[
            "--output",
            "window",
            "--audio-output",
            "device-required",
            "--headless",
            "--chromium",
            "/usr/bin/chromium",
            "--cef-switch",
            "remote-debugging-port=9333",
            "--diagnostics-log",
            "/logs/diagnostics.ndjson",
        ]);
        let properties = stage_properties(&options).expect("staging properties");
        assert!(properties.contains(&"-p:RustyEngineProductRenderOutput=window".to_owned()));
        assert!(properties.contains(&"-p:RustyEngineProductAudioOutput=device-required".to_owned()));
        let host = supervised_host_arguments(
            Path::new("/product"),
            Path::new("/persistence"),
            7,
            &options,
        )
        .expect("host arguments");
        for pair in [
            ["--chromium", "/usr/bin/chromium"],
            ["--cef-switch", "remote-debugging-port=9333"],
            ["--diagnostics-log", "/logs/diagnostics.ndjson"],
        ] {
            assert!(host.windows(2).any(|window| window == pair), "{pair:?}");
        }
        assert!(Arguments::parse(
            ["dev", "--project", "P.csproj", "--output", "tv"].map(str::to_owned)
        )
        .is_err());
        assert!(Arguments::parse(
            ["dev", "--project", "P.csproj", "--chromium", "/c"].map(str::to_owned)
        )
        .is_err());
    }

    #[test]
    fn dev_help_carries_the_project_whose_pin_answers_it() {
        let help = |arguments: &[&str]| match Arguments::parse(
            arguments.iter().map(|value| (*value).to_owned()),
        )
        .expect("help parses")
        .command
        {
            CommandName::DevHelp(project) => project,
            _ => panic!("dev --help is dev help"),
        };
        assert_eq!(help(&["dev", "--help"]), None);
        assert_eq!(
            help(&[
                "dev",
                "--project",
                "src/P.csproj",
                "--output",
                "window",
                "-h"
            ]),
            Some(PathBuf::from("src/P.csproj"))
        );
    }

    #[test]
    fn every_runtime_argument_is_explicit_and_delegation_passes_none() {
        assert!(pinned_selection(&dev_options(&[])));
        for explicit in [
            &[
                "--runtime",
                "/cache/pairs/0.1.0-dev.abc123def456/runtime-pack",
            ][..],
            &["--engine-source", "/engine"][..],
        ] {
            assert!(!pinned_selection(&dev_options(explicit)), "{explicit:?}");
        }
        let delegated = delegated_arguments(&dev_options(&[
            "--output",
            "window",
            "--live-debug",
            "--cef-switch",
            "remote-debugging-port=9333",
        ]));
        assert!(!delegated.iter().any(|argument| argument == "--runtime"));
        let delegated = Arguments::parse(
            delegated
                .iter()
                .map(|argument| argument.to_str().unwrap().to_owned()),
        )
        .expect("the pair's rusty accepts the delegated arguments");
        let CommandName::Dev(delegated) = delegated.command else {
            panic!("dev command");
        };
        assert!(
            pinned_selection(&delegated),
            "the delegated run is the pinned selection"
        );
        assert_eq!(delegated.output.as_deref(), Some("window"));
        assert!(delegated.live_debug);
        assert_eq!(delegated.cef_switches, ["remote-debugging-port=9333"]);
    }

    #[test]
    fn only_the_pinned_selection_swaps_to_the_desktop_pack() {
        let base = env::temp_dir().join(format!("rusty-window-{}", std::process::id()));
        let pair = base.join("pairs/0.1.0-dev.abc123def456");
        let pack = pair.join("runtime-pack");
        fs::create_dir_all(&pack).unwrap();
        fs::write(pair.join("pair-manifest.json"), "{}").unwrap();

        let swapped = desktop_pair_for(&pack, true).expect("the pinned pack swaps");
        assert_eq!(
            swapped.map(|pair| pair.version).as_deref(),
            Some("0.1.0-dev.abc123def456")
        );
        let explicit = desktop_pair_for(&pack, false).expect_err("an explicit pack is not swapped");
        assert!(explicit.contains("RUSTY_DEV_WINDOW"), "{explicit}");

        let loose = base.join("contributor-pack");
        fs::create_dir_all(&loose).unwrap();
        assert!(desktop_pair_for(&loose, true).is_err(), "no pair beside it");

        fs::create_dir_all(loose.join("lib/cef")).unwrap();
        assert!(
            desktop_pair_for(&loose, false).unwrap().is_none(),
            "already a desktop pack"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn supervised_host_arguments_include_distinct_stable_runtime_roots() {
        let arguments = supervised_host_arguments(
            Path::new("/workspace/Product/obj/RustyEngineProduct"),
            Path::new("/workspace/Product/.runtime/persistence"),
            41,
            &dev_options(&[]),
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
            &dev_options(&[]),
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
