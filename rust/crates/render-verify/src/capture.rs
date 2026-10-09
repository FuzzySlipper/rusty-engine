//! Recapturing the lane's scenes from a pair (`rusty-gpu-lane capture`,
//! docs/verification.md#recapturing-scenes). `captures.json` beside
//! `scenes.json` gives each scene file a recipe: the product that shows it
//! and the live-debug steps that set up its view. A capture stages the
//! product on the pair's SDK, runs it headless on the pair's runtime, takes
//! the steps and writes `engine.renderer.snapshot` over the scene file,
//! keeping the one it replaces beside it as `<file>.previous`.
//!
//! A pair is a directory laid out as the `rusty` CLI installs one:
//! `runtime-pack/bin` (the host, `rusty-live-debug`, `rusty-scene-render`)
//! and `sdk-feed/Rusty.Engine.<version>.nupkg`. `scripts/gpu-lane-pair.sh`
//! builds one from an Engine checkout.

use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

pub const CAPTURES_FILE: &str = "captures.json";
/// The product host's line naming where it serves.
const LISTENING: &str = "listening at ";
/// How long a product may take to stage and start before the capture fails.
const START_TIMEOUT: Duration = Duration::from_secs(180);
/// How long the host takes to finish once asked to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(20);
/// Seconds a product settles after it serves and before the first step,
/// unless its recipe says otherwise.
const DEFAULT_SETTLE_SECONDS: f64 = 3.0;

/// An installed (or locally built) Engine SDK/runtime pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Pair {
    pub root: PathBuf,
    pub version: String,
    pub runtime_bin: PathBuf,
    pub feed: PathBuf,
}

impl Pair {
    pub fn open(root: &Path) -> Result<Self, String> {
        let runtime_bin = root.join("runtime-pack").join("bin");
        let feed = root.join("sdk-feed");
        let entries =
            fs::read_dir(&feed).map_err(|error| format!("{}: {error}", feed.display()))?;
        let mut versions: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_prefix("Rusty.Engine.")
                    .and_then(|rest| rest.strip_suffix(".nupkg"))
                    .map(str::to_owned)
            })
            .collect();
        versions.sort();
        let version = match versions.as_slice() {
            [version] => version.clone(),
            [] => {
                return Err(format!(
                    "{}: no Rusty.Engine.<version>.nupkg",
                    feed.display()
                ))
            }
            _ => return Err(format!("{}: more than one SDK package", feed.display())),
        };
        let host = runtime_bin.join(executable("rusty-product-host"));
        if !host.exists() {
            return Err(format!("{}: missing", host.display()));
        }
        Ok(Self {
            root: root.to_owned(),
            version,
            runtime_bin,
            feed,
        })
    }

    pub fn tool(&self, name: &str) -> PathBuf {
        self.runtime_bin.join(executable(name))
    }
}

fn executable(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// One step of a recipe: a live-debug command line, or a pause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    Command(String),
    Wait { wait: f64 },
}

/// How one scene file is captured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capture {
    /// The scene file, relative to the lane directory.
    pub file: String,
    /// The product's project file. `{engine}` stands for the Engine checkout
    /// (`--checkout`), for its fixtures.
    pub project: String,
    /// MSBuild properties the product stages with, beyond the pair's.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    /// Seconds after the product serves before the first step.
    #[serde(default)]
    pub settle: Option<f64>,
    pub steps: Vec<Step>,
    /// Where the recipe came from and what it shows.
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureManifest {
    pub captures: Vec<Capture>,
}

impl CaptureManifest {
    pub fn read(root: &Path) -> Result<Self, String> {
        let path = root.join(CAPTURES_FILE);
        let text =
            fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }
}

/// What a capture run is asked to do.
pub struct CaptureRequest<'a> {
    pub root: &'a Path,
    pub pair: &'a Pair,
    /// The Engine checkout `{engine}` names.
    pub checkout: Option<&'a Path>,
    /// Only these scene files (by file name or path), when not empty.
    pub only: &'a [String],
    /// Scratch space: staged products, persistence roots, logs.
    pub work: &'a Path,
}

/// How one capture went.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureOutcome {
    pub file: String,
    pub captured: bool,
    pub detail: String,
}

/// Captures every selected recipe in turn; a failed one leaves its scene as
/// it was. A selection naming no recipe is an error, so nothing passes
/// unnoticed.
pub fn capture(request: &CaptureRequest<'_>) -> Result<Vec<CaptureOutcome>, String> {
    let manifest = CaptureManifest::read(request.root)?;
    let selected = |capture: &Capture| {
        request.only.is_empty()
            || request.only.iter().any(|name| {
                capture.file == *name
                    || Path::new(&capture.file)
                        .file_stem()
                        .is_some_and(|stem| stem.to_string_lossy() == *name)
            })
    };
    let unmatched: Vec<&String> = request
        .only
        .iter()
        .filter(|name| {
            !manifest.captures.iter().any(|capture| {
                capture.file == **name
                    || Path::new(&capture.file)
                        .file_stem()
                        .is_some_and(|stem| stem.to_string_lossy() == name.as_str())
            })
        })
        .collect();
    if !unmatched.is_empty() {
        return Err(format!(
            "no capture recipe for {}",
            unmatched
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    fs::create_dir_all(request.work)
        .map_err(|error| format!("{}: {error}", request.work.display()))?;
    let mut outcomes = Vec::new();
    for recipe in manifest.captures.iter().filter(|capture| selected(capture)) {
        eprintln!("rusty-gpu-lane: capturing {}", recipe.file);
        let outcome = match capture_one(request, recipe) {
            Ok(detail) => CaptureOutcome {
                file: recipe.file.clone(),
                captured: true,
                detail,
            },
            Err(error) => CaptureOutcome {
                file: recipe.file.clone(),
                captured: false,
                detail: error,
            },
        };
        eprintln!(
            "rusty-gpu-lane: {} {}: {}",
            outcome.file,
            if outcome.captured {
                "captured"
            } else {
                "FAILED"
            },
            outcome.detail
        );
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// The project path with `{engine}` replaced.
pub fn project_path(recipe: &Capture, checkout: Option<&Path>) -> Result<PathBuf, String> {
    if recipe.project.contains("{engine}") {
        let checkout = checkout.ok_or_else(|| {
            format!(
                "{}: the recipe names {{engine}}: pass --checkout",
                recipe.file
            )
        })?;
        return Ok(PathBuf::from(
            recipe
                .project
                .replace("{engine}", &checkout.display().to_string()),
        ));
    }
    Ok(PathBuf::from(&recipe.project))
}

fn capture_one(request: &CaptureRequest<'_>, recipe: &Capture) -> Result<String, String> {
    let pair = request.pair;
    let project = project_path(recipe, request.checkout)?;
    let name = Path::new(&recipe.file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "scene".to_owned());
    let work = request.work.join(&name);
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).map_err(|error| format!("{}: {error}", work.display()))?;
    let staged = work.join("product");
    stage(pair, &project, &staged, &recipe.properties, request.work)?;

    let persistence = work.join("persist");
    fs::create_dir_all(&persistence)
        .map_err(|error| format!("{}: {error}", persistence.display()))?;
    let mut host = Command::new(pair.tool("rusty-product-host"));
    host.arg("--product")
        .arg(&staged)
        .args(["--loader", "coreclr", "--headless"])
        .arg("--persistence-root")
        .arg(&persistence)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(root) = dotnet_root() {
        host.env("DOTNET_ROOT", root);
    }
    let mut child = host
        .spawn()
        .map_err(|error| format!("{}: {error}", pair.tool("rusty-product-host").display()))?;
    let result = (|| {
        let origin = wait_for_origin(&mut child, &work.join("host.log"))?;
        pause(recipe.settle.unwrap_or(DEFAULT_SETTLE_SECONDS));
        for step in &recipe.steps {
            match step {
                Step::Wait { wait } => pause(*wait),
                Step::Command(line) => {
                    live_debug(pair, &origin, line)?;
                }
            }
        }
        let snapshot = work.join("scene.rscene");
        live_debug(
            pair,
            &origin,
            &format!("engine.renderer.snapshot {}", snapshot.display()),
        )?;
        if !snapshot.exists() {
            return Err(format!(
                "{}: the snapshot was not written",
                snapshot.display()
            ));
        }
        Ok(snapshot)
    })();
    stop(&mut child);
    let snapshot = result?;
    let target = request.root.join(&recipe.file);
    if target.exists() {
        let previous = request.root.join(format!("{}.previous", recipe.file));
        fs::rename(&target, &previous)
            .map_err(|error| format!("{}: {error}", previous.display()))?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    fs::copy(&snapshot, &target).map_err(|error| format!("{}: {error}", target.display()))?;
    let bytes = fs::metadata(&target).map(|meta| meta.len()).unwrap_or(0);
    Ok(format!("{bytes} bytes on pair {}", pair.version))
}

/// Builds and stages `project` on the pair's SDK, live debug admitted, into
/// `staged`. Packages restore into the work directory, so the pair's SDK is
/// the one used.
fn stage(
    pair: &Pair,
    project: &Path,
    staged: &Path,
    properties: &BTreeMap<String, String>,
    work: &Path,
) -> Result<(), String> {
    let mut common = vec![
        format!("-p:RustyEnginePackageVersion={}", pair.version),
        format!("-p:RustyEngineFixtureSdkVersion={}", pair.version),
        format!("-p:RestoreAdditionalProjectSources={}", pair.feed.display()),
        "-p:RustyEngineProductLiveDebug=true".to_owned(),
        format!("-p:RustyEngineStagedProductDirectory={}", staged.display()),
    ];
    common.extend(
        properties
            .iter()
            .map(|(key, value)| format!("-p:{key}={value}")),
    );
    for target in [None, Some("StageRustyEngineCoreClrProduct")] {
        let mut command = Command::new("dotnet");
        match target {
            None => command.arg("build"),
            Some(target) => command.arg("msbuild").arg(format!("-t:{target}")),
        };
        command
            .arg(project)
            .args(&common)
            .env("NUGET_PACKAGES", work.join("nuget"))
            .env("DOTNET_CLI_HOME", work.join("dotnet"));
        let output = command
            .output()
            .map_err(|error| format!("dotnet: {error}"))?;
        if !output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let errors: Vec<&str> = text
                .lines()
                .filter(|line| line.contains("error"))
                .take(4)
                .collect();
            return Err(format!(
                "staging {} failed: {}",
                project.display(),
                errors.join(" | ")
            ));
        }
    }
    if !staged.join("product.json").exists() {
        return Err(format!("{}: nothing staged", staged.display()));
    }
    Ok(())
}

/// The directory of the `dotnet` on PATH, for the host's CoreCLR loader.
fn dotnet_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("DOTNET_ROOT") {
        return Some(PathBuf::from(root));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(executable("dotnet")))
        .find(|candidate| candidate.exists())
        .and_then(|dotnet| fs::canonicalize(dotnet).ok())
        .and_then(|dotnet| dotnet.parent().map(Path::to_path_buf))
}

/// Reads the host's output into `log` until it says where it serves.
fn wait_for_origin(child: &mut Child, log: &Path) -> Result<String, String> {
    let (sender, receiver) = mpsc::channel::<String>();
    for stream in [
        child
            .stdout
            .take()
            .map(|out| Box::new(out) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|err| Box::new(err) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let sender = sender.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
    }
    drop(sender);
    let log_path = log.to_owned();
    let started = Instant::now();
    let mut lines = Vec::new();
    while started.elapsed() < START_TIMEOUT {
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(line) => {
                let origin = line
                    .find(LISTENING)
                    .map(|at| line[at + LISTENING.len()..].trim().to_owned());
                lines.push(line);
                if let Some(origin) = origin {
                    // Keep logging in the background for diagnosis.
                    std::thread::spawn(move || {
                        let mut all = lines;
                        while let Ok(line) = receiver.recv() {
                            all.push(line);
                            let _ = fs::write(&log_path, all.join("\n"));
                        }
                    });
                    return Ok(origin);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if let Ok(Some(status)) = child.try_wait() {
            let _ = fs::write(log, lines.join("\n"));
            return Err(format!(
                "the host exited {status} before serving: {}",
                lines
                    .iter()
                    .rev()
                    .take(4)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
    }
    let _ = fs::write(log, lines.join("\n"));
    Err(format!("the host did not serve within {START_TIMEOUT:?}"))
}

fn live_debug(pair: &Pair, origin: &str, line: &str) -> Result<String, String> {
    let output = Command::new(pair.tool("rusty-live-debug"))
        .args(["--origin", origin, "--command", line])
        .output()
        .map_err(|error| format!("rusty-live-debug: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        return Err(format!(
            "`{line}` exited {}: {}{}",
            output.status,
            text.trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(text)
}

fn pause(seconds: f64) {
    if seconds > 0.0 {
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }
}

/// Asks the host to stop (SIGTERM on Unix, so it stops the runtime and the
/// headless browser it started), and kills it if it does not.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
        let started = Instant::now();
        while started.elapsed() < STOP_TIMEOUT {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_read_commands_and_waits_and_refuse_unknown_fields() {
        let manifest: CaptureManifest = serde_json::from_str(
            r#"{"captures":[{"file":"scenes/a.rscene","project":"{engine}/fixtures/x/X.csproj","settle":2,"steps":["lighting.sky 0.3",{"wait":4.5},"lighting.backdrop 1000"]}]}"#,
        )
        .unwrap();
        let recipe = &manifest.captures[0];
        assert_eq!(
            recipe.steps,
            [
                Step::Command("lighting.sky 0.3".to_owned()),
                Step::Wait { wait: 4.5 },
                Step::Command("lighting.backdrop 1000".to_owned()),
            ]
        );
        assert_eq!(
            project_path(recipe, Some(Path::new("/src/engine"))).unwrap(),
            PathBuf::from("/src/engine/fixtures/x/X.csproj")
        );
        assert!(
            project_path(recipe, None).is_err(),
            "{{engine}} needs a checkout"
        );
        assert!(serde_json::from_str::<CaptureManifest>(
            r#"{"captures":[{"file":"a","project":"b","steps":[],"camera":1}]}"#
        )
        .is_err());
    }

    #[test]
    fn a_pair_is_its_runtime_pack_and_one_sdk_package() {
        let root = std::env::temp_dir().join(format!("render-verify-pair-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("runtime-pack/bin")).unwrap();
        fs::create_dir_all(root.join("sdk-feed")).unwrap();
        assert!(Pair::open(&root).is_err(), "no package");
        fs::write(root.join("sdk-feed/Rusty.Engine.0.1.0-dev.abc.nupkg"), b"").unwrap();
        assert!(Pair::open(&root).is_err(), "no host");
        fs::write(
            root.join("runtime-pack/bin")
                .join(executable("rusty-product-host")),
            b"",
        )
        .unwrap();
        let pair = Pair::open(&root).unwrap();
        assert_eq!(pair.version, "0.1.0-dev.abc");
        assert_eq!(pair.feed, root.join("sdk-feed"));
        fs::write(root.join("sdk-feed/Rusty.Engine.0.1.0-dev.def.nupkg"), b"").unwrap();
        assert!(Pair::open(&root).is_err(), "two packages are ambiguous");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_selection_naming_no_recipe_is_refused() {
        let root =
            std::env::temp_dir().join(format!("render-verify-capture-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("pair/runtime-pack/bin")).unwrap();
        fs::create_dir_all(root.join("pair/sdk-feed")).unwrap();
        fs::write(root.join("pair/sdk-feed/Rusty.Engine.1.nupkg"), b"").unwrap();
        fs::write(
            root.join("pair/runtime-pack/bin")
                .join(executable("rusty-product-host")),
            b"",
        )
        .unwrap();
        fs::write(
            root.join(CAPTURES_FILE),
            r#"{"captures":[{"file":"scenes/a.rscene","project":"x","steps":[]}]}"#,
        )
        .unwrap();
        let pair = Pair::open(&root.join("pair")).unwrap();
        let error = capture(&CaptureRequest {
            root: &root,
            pair: &pair,
            checkout: None,
            only: &["b".to_owned()],
            work: &root.join("work"),
        })
        .unwrap_err();
        assert!(error.contains("no capture recipe for b"), "{error}");
        let _ = fs::remove_dir_all(&root);
    }
}
