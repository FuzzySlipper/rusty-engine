//! The intentionally thin Product iteration entry point.
//!
//! This binary does not contain Product configuration. The SDK evaluates and
//! stages that truth, while a runtime pack supplies the exact host it starts.

use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime},
};

use serde::Deserialize;
use serde_json::Value;

const STAGE_TARGET: &str = "StageRustyEngineCoreClrProduct";
const STAGED_PRODUCT_PROPERTY: &str = "RustyEngineStagedProductDirectory";
const WATCH_PATHS_PROPERTY: &str = "RustyEngineWatchPaths";
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

fn main() -> Result<(), String> {
    let arguments = Arguments::parse(env::args().skip(1))?;
    match arguments.command {
        CommandName::Dev(options) => dev(options),
    }
}

fn dev(options: DevOptions) -> Result<(), String> {
    let runtime = RuntimePack::resolve(&options)?;
    runtime.verify()?;

    let persistence_root = development_persistence_root(&options.project)?;
    let content_store_root = development_content_store_root(&options.project)?;
    let mut staged = stage_product(&options)?;
    verify_staged_product(&staged)?;
    let mut watches = query_watch_paths(&options.project)?;
    let mut snapshot = FileSnapshot::capture(&watches)?;
    let mut child = Some(SupervisedHost::start(
        &runtime.host,
        &staged,
        &persistence_root,
        &content_store_root,
        options.debugger,
    )?);
    let mut crash_budget = CrashBudget::new(MAX_UNEXPECTED_EXITS_PER_ARTIFACT);

    diagnostic(
        "started",
        serde_json::json!({
            "project": options.project,
            "productDirectory": staged,
            "persistenceRoot": persistence_root,
            "contentStoreRoot": content_store_root,
            "runtimePack": runtime.root,
            "loader": "coreclr",
            "watchPaths": watches,
            "pid": child.as_ref().expect("initial child is present").child.id(),
            "runtimeInstanceId": child.as_ref().expect("initial child is present").runtime_instance_id,
        }),
    );

    loop {
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
                            &content_store_root,
                            options.debugger,
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
        if !record_observed_snapshot(&mut snapshot, next) {
            continue;
        }

        diagnostic(
            "change-detected",
            serde_json::json!({ "watchPaths": watches }),
        );
        let next_staged = match stage_product(&options) {
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
        let refreshed_watches = match query_watch_paths(&options.project) {
            Ok(watches) => watches,
            Err(error) => {
                diagnostic(
                    "restage-failed",
                    serde_json::json!({
                        "phase": "query-watch-paths",
                        "error": error,
                    }),
                );
                continue;
            }
        };
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
                &content_store_root,
                options.debugger,
            )?);
            true
        };
        if started_after_restage && child.is_none() {
            child = Some(SupervisedHost::start(
                &runtime.host,
                &next_staged,
                &persistence_root,
                &content_store_root,
                options.debugger,
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
                    "contentStoreRoot": content_store_root,
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
                    "contentStoreRoot": content_store_root,
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
    Dev(DevOptions),
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
}

impl Arguments {
    fn parse(values: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut values = values.into_iter();
        let command = values.next().ok_or_else(usage)?;
        if command != "dev" {
            return Err(usage());
        }
        let mut project = None;
        let mut runtime = None;
        let mut engine_source = None;
        let mut bind_host = None;
        let mut port = None;
        let mut live_debug = false;
        let mut debugger = false;
        while let Some(value) = values.next() {
            match value.as_str() {
                "--project" => {
                    project = Some(PathBuf::from(required_value(&mut values, "--project")?))
                }
                "--runtime" => {
                    runtime = Some(PathBuf::from(required_value(&mut values, "--runtime")?))
                }
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
                "--help" => return Err(usage()),
                _ => {
                    return Err(format!(
                        "RUSTY_DEV_ARGUMENT: unknown argument `{value}`\n{}",
                        usage()
                    ))
                }
            }
        }
        if runtime.is_some() && engine_source.is_some() {
            return Err(
                "RUSTY_DEV_ARGUMENT: --runtime and --engine-source are mutually exclusive"
                    .to_owned(),
            );
        }
        let project = project.ok_or_else(|| {
            "RUSTY_DEV_ARGUMENT: --project <ordinary-product.csproj> is required".to_owned()
        })?;
        Ok(Self {
            command: CommandName::Dev(DevOptions {
                project,
                runtime,
                engine_source,
                bind_host,
                port,
                live_debug,
                debugger,
            }),
        })
    }
}

fn required_value(values: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    values
        .next()
        .ok_or_else(|| format!("RUSTY_DEV_ARGUMENT: {flag} requires a value"))
}

fn usage() -> String {
    "usage: rusty dev --project <ordinary-product.csproj> [--runtime <runtime-pack>] [--engine-source <rusty-engine-source>] [--bind-host <IPv4>] [--port <u16>] [--live-debug] [--debugger]\n\nCoreCLR is the only normal loader. --debugger disables supervised worker startup/callback deadlines for managed breakpoints; source changes still replace workers. The SDK stages Product truth; this command never invokes Cargo or auto-discovers an adjacent Engine checkout. Use an explicit override only for Engine contributor runtime packs.".to_owned()
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
            runtime_beside_current_executable()?
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

fn runtime_beside_current_executable() -> Result<PathBuf, String> {
    let executable = env::current_exe().map_err(|error| {
        format!("RUSTY_DEV_RUNTIME_LOCATE: cannot resolve rusty executable: {error}")
    })?;
    let bin = executable.parent().ok_or_else(|| {
        "RUSTY_DEV_RUNTIME_LOCATE: rusty executable has no parent directory".to_owned()
    })?;
    let root = bin.parent().ok_or_else(|| "RUSTY_DEV_RUNTIME_LOCATE: rusty executable must reside in a runtime-pack bin directory; use --runtime or --engine-source for contributor work".to_owned())?;
    if root.join("runtime-manifest.json").is_file() {
        Ok(root.to_owned())
    } else {
        Err("RUSTY_DEV_RUNTIME_LOCATE: no runtime-manifest.json beside rusty; use --runtime <runtime-pack> or --engine-source <rusty-engine-source>. Normal operation never searches for an adjacent checkout.".to_owned())
    }
}

fn stage_product(options: &DevOptions) -> Result<PathBuf, String> {
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
    let properties = stage_properties(options)?;
    let mut build_arguments = vec!["build".to_owned(), project_argument];
    build_arguments.extend(properties.iter().cloned());
    run_dotnet(&build_arguments)?;
    let staged = query_msbuild_property(
        &project,
        Some(STAGE_TARGET),
        STAGED_PRODUCT_PROPERTY,
        &properties,
    )?;
    let staged = PathBuf::from(staged);
    absolute(&staged)
}

fn query_watch_paths(project: &Path) -> Result<Vec<PathBuf>, String> {
    let value = query_msbuild_property(project, None, WATCH_PATHS_PROPERTY, &[])?;
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

fn query_msbuild_property(
    project: &Path,
    target: Option<&str>,
    property: &str,
    properties: &[String],
) -> Result<String, String> {
    let project = project
        .to_str()
        .ok_or("RUSTY_DEV_PROJECT: project path must be UTF-8")?;
    let mut arguments = vec![
        "msbuild".to_owned(),
        project.to_owned(),
        "-nologo".to_owned(),
        "-verbosity:quiet".to_owned(),
    ];
    if let Some(target) = target {
        arguments.push(format!("-t:{target}"));
    }
    arguments.extend(properties.iter().cloned());
    arguments.push(format!("-getProperty:{property}"));
    let output = Command::new("dotnet")
        .args(&arguments)
        .output()
        .map_err(|error| format!("RUSTY_DEV_DOTNET: could not start dotnet msbuild: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "RUSTY_DEV_MSBUILD: property {property} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| format!("RUSTY_DEV_MSBUILD: property {property} output was not UTF-8"))?
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("RUSTY_DEV_MSBUILD: property {property} produced no value"))
}

fn stage_properties(options: &DevOptions) -> Result<Vec<String>, String> {
    let mut properties = Vec::new();
    if let Some(engine_source) = &options.engine_source {
        let engine_source = absolute(engine_source)?;
        let engine_source = engine_source
            .to_str()
            .ok_or("RUSTY_DEV_ENGINE_SOURCE: Engine source path must be UTF-8")?;
        properties.push("-p:RustyEngineUseSourceDevelopment=true".to_owned());
        properties.push(format!(
            "-p:RustyEngineSourceDevelopmentPath={engine_source}"
        ));
    }
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

fn development_content_store_root(project: &Path) -> Result<PathBuf, String> {
    Ok(development_runtime_root(project)?.join("content-store"))
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
        content_store_root: &Path,
        debugger: bool,
    ) -> Result<Self, String> {
        let runtime_instance_id = next_supervised_runtime_instance_id()?;
        let arguments = supervised_host_arguments(
            product,
            persistence_root,
            content_store_root,
            runtime_instance_id,
            debugger,
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

    fn try_wait(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .try_wait()
            .map_err(|error| format!("RUSTY_DEV_CHILD_WAIT: {error}"))
    }

    /// Replaces only the disposable runtime inside a live supervised shell.
    /// The browser listener, diagnostics, and persistent Engine roots remain
    /// fixed process configuration for the duration of `rusty dev`.
    fn replace_runtime(&mut self, product: &Path) -> Result<(), String> {
        let command = encode_runtime_replacement_command(product)?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or("RUSTY_DEV_CHILD_REPLACE: supervised child stdin was unavailable")?;
        stdin
            .write_all(&command)
            .and_then(|_| stdin.flush())
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_CHILD_REPLACE: could not send replacement configuration: {error}"
                )
            })?;
        Ok(())
    }
}

/// The only command the long-lived `rusty dev` shell accepts after startup.
/// It intentionally carries no generic method name, options bag, or
/// compatibility negotiation: successful staging can replace exactly one C#
/// runtime incarnation with the next staged Product directory.
#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SupervisedHostCommand {
    #[serde(rename_all = "camelCase")]
    ReplaceRuntime { product_directory: String },
}

fn encode_runtime_replacement_command(product: &Path) -> Result<Vec<u8>, String> {
    let product_directory = product
        .to_str()
        .ok_or("RUSTY_DEV_STAGE: staged product path must be UTF-8")?
        .to_owned();
    let payload = serde_json::to_vec(&SupervisedHostCommand::ReplaceRuntime { product_directory })
        .map_err(|error| {
            format!(
                "RUSTY_DEV_CHILD_REPLACE: replacement configuration could not be encoded: {error}"
            )
        })?;
    if payload.len() > MAX_SUPERVISOR_COMMAND_BYTES {
        return Err(
            "RUSTY_DEV_CHILD_REPLACE: replacement configuration exceeds its bounded command length"
                .to_owned(),
        );
    }
    let length = u32::try_from(payload.len()).map_err(|_| {
        "RUSTY_DEV_CHILD_REPLACE: replacement configuration length cannot be represented".to_owned()
    })?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn supervised_host_arguments(
    product: &Path,
    persistence_root: &Path,
    content_store_root: &Path,
    runtime_instance_id: u64,
    debugger: bool,
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
        "--content-store-root".to_owned(),
        content_store_root
            .to_str()
            .ok_or("RUSTY_DEV_CONTENT_STORE: content store root path must be UTF-8")?
            .to_owned(),
    ];
    if debugger {
        arguments.push("--debugger".to_owned());
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
        let CommandName::Dev(options) = arguments.command;
        assert!(options.debugger);
        assert!(!stage_properties(&options)
            .expect("staging properties")
            .iter()
            .any(|property| property.contains("debugger")));
        let host = supervised_host_arguments(
            Path::new("/product"),
            Path::new("/persistence"),
            Path::new("/content-store"),
            7,
            options.debugger,
        )
        .expect("host arguments");
        assert!(host.iter().any(|argument| argument == "--debugger"));
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
        let CommandName::Dev(options) = arguments.command;
        assert_eq!(options.project, PathBuf::from("Product.csproj"));
        assert_eq!(options.runtime, Some(PathBuf::from("/runtime-pack")));
        assert_eq!(options.bind_host.as_deref(), Some("127.0.0.1"));
        assert_eq!(options.port, Some(9348));
        assert!(options.live_debug);
        assert!(!options.debugger);
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
        let content_store_root = development_content_store_root(Path::new("src/Product.csproj"))
            .expect("development content store root");
        let current_directory = env::current_dir().expect("current directory");
        let repository_root = current_directory
            .ancestors()
            .find(|candidate| candidate.join(".git").exists())
            .expect("test runs from the Engine repository");

        assert_eq!(
            persistence_root,
            repository_root.join(".runtime").join("persistence")
        );
        assert_eq!(
            content_store_root,
            repository_root.join(".runtime").join("content-store")
        );
        assert!(persistence_root.is_absolute());
        assert!(content_store_root.is_absolute());
        assert_ne!(persistence_root, content_store_root);
    }

    #[test]
    fn development_runtime_roots_for_a_loose_project_do_not_follow_staged_output() {
        let persistence_root =
            development_persistence_root(Path::new("/workspace/Product/Product.csproj"))
                .expect("development persistence root");
        let content_store_root =
            development_content_store_root(Path::new("/workspace/Product/Product.csproj"))
                .expect("development content store root");
        let staged_product = Path::new("/workspace/Product/obj/RustyEngineProduct");

        assert_eq!(
            persistence_root,
            PathBuf::from("/workspace/Product/.runtime/persistence")
        );
        assert_eq!(
            content_store_root,
            PathBuf::from("/workspace/Product/.runtime/content-store")
        );
        assert_ne!(
            persistence_root,
            staged_product.join(".runtime/persistence")
        );
        assert_ne!(
            content_store_root,
            staged_product.join(".runtime/content-store")
        );
    }

    #[test]
    fn supervised_host_arguments_include_distinct_stable_runtime_roots() {
        let arguments = supervised_host_arguments(
            Path::new("/workspace/Product/obj/RustyEngineProduct"),
            Path::new("/workspace/Product/.runtime/persistence"),
            Path::new("/workspace/Product/.runtime/content-store"),
            41,
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
            "--content-store-root",
            "/workspace/Product/.runtime/content-store",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert_eq!(arguments, expected);
    }

    #[test]
    fn restage_encodes_the_one_bounded_runtime_replacement_command() {
        let frame = encode_runtime_replacement_command(Path::new(
            "/workspace/Product/obj/RustyEngineProduct",
        ))
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
    fn initial_supervised_shell_rejects_a_zero_runtime_seed() {
        let error = supervised_host_arguments(
            Path::new("/workspace/Product"),
            Path::new("/workspace/Product/.runtime/persistence"),
            Path::new("/workspace/Product/.runtime/content-store"),
            0,
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
