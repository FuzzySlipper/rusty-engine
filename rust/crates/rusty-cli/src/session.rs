//! Dev sessions. Every `rusty dev`, in the foreground or started with
//! `rusty dev start`, registers in the machine's session registry
//! (`<rusty cache>/sessions/<id>/`), whatever checkout or pair it runs, so
//! `rusty dev list` shows every product host on the machine and
//! `rusty dev stop <id|port>` stops one without searching processes.
//! Each also holds its project's `.runtime/dev/<project>[#<instance>]/`
//! lock, so one project runs one session unless `--instance` names others
//! (crew playtest runs one host per playtest session); a background session
//! keeps its log there. A session that ends for a reason a caller must see
//! (idle expiry, an unusable port, a removed project) leaves its record,
//! shown by `rusty dev list --all` for a day.
//!
//! Each running `rusty dev` holds an exclusive lock on its `lock` files for
//! its whole life, so a directory whose lock is free belongs to one that died
//! and is removed. Creating its `stop` file asks it to shut down: its watch
//! loop sees the file and closes the host's stdin as Ctrl+C would, so the
//! product is disposed. A `keep` file marks a session to be kept.

use std::{
    ffi::OsString,
    fs::{self, File, TryLockError},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdout, Command, ExitCode, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde_json::{json, Map, Value};

const POLL: Duration = Duration::from_millis(250);
const STOP_TIMEOUT: Duration = Duration::from_secs(300);
const LOG_TAIL_LINES: usize = 40;
/// What the product host prints once it serves.
const LISTENING: &str = " product host listening at ";
/// What the product-host supervisor prints when its runtime or headless
/// browser starts: `{"runtime": <pgid>, "browser": <pgid>}`.
const PROCESSES: &str = "RUSTY_HOST processes=";

pub struct SessionPaths {
    directory: PathBuf,
    record: PathBuf,
    lock: PathBuf,
    stop: PathBuf,
    log: PathBuf,
}

impl SessionPaths {
    /// The session of the project at `project`: equivalent spellings of one
    /// project file (relative, `..`, symlinks) name the same session, and
    /// distinct project files in one repository name distinct ones.
    pub fn for_project(project: &Path, instance: Option<&str>) -> Result<Self, String> {
        let project = super::absolute(project)?;
        // Windows canonicalizes a mapped drive to its UNC share
        // (`P:\x` to `\\?\UNC\server\share\x`), which would key the session
        // apart from the dev state `rusty status` reports for `P:`.
        #[cfg(windows)]
        let project = std::path::absolute(&project).unwrap_or(project);
        #[cfg(not(windows))]
        let project = fs::canonicalize(&project).unwrap_or(project);
        let roots = super::DevelopmentRoots::of(&project)?;
        let relative = project.strip_prefix(&roots.checkout).unwrap_or(&project);
        if relative.file_name().is_none() {
            return Err(format!(
                "RUSTY_DEV_PROJECT: `{}` names no project file",
                project.display()
            ));
        }
        let mut key = session_key(relative);
        if let Some(instance) = instance {
            key = format!("{key}%23{instance}");
        }
        let directory = roots.runtime.join("dev").join(key);
        Ok(Self {
            record: directory.join("session.json"),
            lock: directory.join("lock"),
            stop: directory.join("stop"),
            log: directory.join("dev.log"),
            directory,
        })
    }

    /// The running session's record, or `None`. A record left by a
    /// supervisor that no longer holds the lock is removed.
    fn running(&self) -> Result<Option<Value>, String> {
        let Ok(lock) = File::open(&self.lock) else {
            return Ok(None);
        };
        match lock.try_lock() {
            Ok(()) => {
                let _ = fs::remove_file(&self.record);
                let _ = fs::remove_file(&self.stop);
                Ok(None)
            }
            Err(TryLockError::WouldBlock) => Ok(Some(
                fs::read(&self.record)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .unwrap_or_else(|| json!({ "state": "starting" })),
            )),
            Err(TryLockError::Error(error)) => Err(format!(
                "RUSTY_DEV_SESSION: could not read `{}`: {error}",
                self.lock.display()
            )),
        }
    }

    fn log_tail(&self) -> String {
        let mut text = String::new();
        let _ = File::open(&self.log).and_then(|mut file| file.read_to_string(&mut text));
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n")
    }
}

/// How long `rusty dev stop` waits for a graceful stop before signalling.
const GRACEFUL_STOP: Duration = Duration::from_secs(60);
/// How long a signalled `rusty dev` gets before it is killed.
const TERMINATE_GRACE: Duration = Duration::from_secs(15);
/// A registry directory without its lock yet is a session being claimed.
const CLAIM_WINDOW: Duration = Duration::from_secs(10);

/// One session's files in the machine registry.
struct EntryPaths {
    directory: PathBuf,
    record: PathBuf,
    lock: PathBuf,
    stop: PathBuf,
    keep: PathBuf,
    /// Its time is the session's last use (the product host's
    /// `--activity-file`).
    activity: PathBuf,
}

impl EntryPaths {
    fn new(directory: PathBuf) -> Self {
        Self {
            record: directory.join("session.json"),
            lock: directory.join("lock"),
            stop: directory.join("stop"),
            keep: directory.join("keep"),
            activity: directory.join("activity"),
            directory,
        }
    }

    /// Whether its `rusty dev` still holds the lock.
    fn alive(&self) -> bool {
        File::open(&self.lock)
            .is_ok_and(|lock| matches!(lock.try_lock(), Err(TryLockError::WouldBlock)))
    }

    fn record(&self) -> Value {
        let mut record = fs::read(&self.record)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_else(|| json!({ "state": "starting" }));
        record["keep"] = self.keep.exists().into();
        if let Ok(used) = fs::metadata(&self.activity).and_then(|metadata| metadata.modified()) {
            record["lastActivity"] = epoch_seconds(used).into();
            record["idleSeconds"] = used.elapsed().unwrap_or_default().as_secs().into();
        }
        record
    }

    fn idle(&self) -> Option<Duration> {
        let used = fs::metadata(&self.activity)
            .and_then(|metadata| metadata.modified())
            .ok()?;
        Some(used.elapsed().unwrap_or_default())
    }
}

/// The machine's running dev sessions.
pub struct Registry {
    root: PathBuf,
}

impl Registry {
    pub fn machine() -> Result<Self, String> {
        Ok(Self {
            root: super::pair::cache_root()?.join("sessions"),
        })
    }

    /// The running sessions, oldest first, and how many dead ones were
    /// removed on the way.
    fn live(&self) -> (Vec<(EntryPaths, Value)>, usize) {
        let scan = self.scan();
        (scan.live, scan.removed)
    }

    /// Running sessions, sessions that ended with a reason to report (kept
    /// for [`ENDED_RETENTION`]), and how many dead entries were removed.
    fn scan(&self) -> Scan {
        let mut live = Vec::new();
        let mut ended = Vec::new();
        let mut removed = 0;
        for entry in fs::read_dir(&self.root).into_iter().flatten().flatten() {
            let paths = EntryPaths::new(entry.path());
            if !paths.directory.is_dir() {
                continue;
            }
            let lock = match File::open(&paths.lock) {
                Ok(lock) => lock,
                Err(_) => {
                    let claiming = entry
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .is_ok_and(|modified| {
                            modified.elapsed().unwrap_or_default() < CLAIM_WINDOW
                        });
                    if !claiming && fs::remove_dir_all(&paths.directory).is_ok() {
                        removed += 1;
                    }
                    continue;
                }
            };
            match lock.try_lock() {
                Ok(()) => {
                    drop(lock);
                    let record = paths.record();
                    let ended_at = record["endedAt"].as_u64();
                    let recent = ended_at.is_some_and(|ended| {
                        epoch_seconds(std::time::SystemTime::now()).saturating_sub(ended)
                            < ENDED_RETENTION.as_secs()
                    });
                    if recent {
                        ended.push(record);
                    } else if fs::remove_dir_all(&paths.directory).is_ok() {
                        removed += 1;
                    }
                }
                Err(TryLockError::WouldBlock) => {
                    let record = paths.record();
                    live.push((paths, record));
                }
                Err(TryLockError::Error(_)) => {}
            }
        }
        live.sort_by_key(|(_, record)| record["startedAt"].as_u64().unwrap_or(0));
        ended.sort_by_key(|record| record["endedAt"].as_u64().unwrap_or(0));
        Scan {
            live,
            ended,
            removed,
        }
    }

    /// The running session named by its id or the port it serves on.
    fn find(&self, target: &str) -> Result<(EntryPaths, Value), String> {
        let port = target.parse::<u16>().ok();
        self.live()
            .0
            .into_iter()
            .find(|(_, record)| {
                record["id"] == target
                    || port.is_some_and(|port| record["port"].as_u64() == Some(u64::from(port)))
            })
            .ok_or_else(|| {
                format!(
                    "RUSTY_DEV_SESSION: no running session has id or port `{target}`; `rusty dev list` shows them"
                )
            })
    }
}

struct Scan {
    live: Vec<(EntryPaths, Value)>,
    ended: Vec<Value>,
    removed: usize,
}

/// How long the record of a session that ended for a reason stays listed.
const ENDED_RETENTION: Duration = Duration::from_secs(24 * 3600);

fn epoch_seconds(time: std::time::SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A short readable id: the project file's name, its instance and the
/// `rusty dev` pid.
fn session_id(project: &Path, instance: Option<&str>) -> String {
    let stem: String = project
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '.' {
                character
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches('-');
    match instance {
        Some(instance) => format!("{stem}.{instance}-{}", std::process::id()),
        None => format!("{stem}-{}", std::process::id()),
    }
}

/// One directory name per project path within the repository: `/` and `%`
/// are escaped, so `a/b.csproj` and `a%2Fb.csproj` stay distinct.
fn session_key(relative: &Path) -> String {
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('%', "%25"))
        .collect::<Vec<_>>()
        .join("%2F")
}

/// What a `rusty dev` registers about itself.
pub struct Registration<'a> {
    pub project: &'a Path,
    pub persistence_root: &'a Path,
    /// Another concurrent session of the same project (`--instance`).
    pub instance: Option<&'a str>,
    /// The checkout the project is in.
    pub checkout: &'a Path,
    /// Started by `rusty dev start`, with its log in the project's
    /// `.runtime/dev/`.
    pub background: bool,
    pub label: Option<&'a str>,
    pub keep: bool,
}

/// The supervisor's side: the locks it holds and the record it keeps current
/// in the registry and, for a background session, its project directory.
pub struct Session {
    entry: EntryPaths,
    project: SessionPaths,
    record: Mutex<Map<String, Value>>,
    /// Ended for a reason a caller must see: the record stays.
    retained: std::sync::atomic::AtomicBool,
    _locks: Vec<File>,
}

impl Session {
    pub fn claim(registration: &Registration<'_>) -> Result<Arc<Self>, String> {
        Self::claim_in(&Registry::machine()?, registration)
    }

    fn claim_in(registry: &Registry, registration: &Registration<'_>) -> Result<Arc<Self>, String> {
        let Registration {
            project,
            persistence_root,
            instance,
            checkout,
            background,
            label,
            keep,
        } = *registration;
        let mut locks = Vec::new();
        let (project_paths, project_lock) = claim_project(project, instance)?;
        locks.push(project_lock);
        // Dead sessions are cleared whenever one starts.
        let _ = registry.live();
        let entry = EntryPaths::new(registry.root.join(session_id(project, instance)));
        fs::create_dir_all(&entry.directory).map_err(|error| {
            format!(
                "RUSTY_DEV_SESSION: could not create `{}`: {error}",
                entry.directory.display()
            )
        })?;
        let lock = File::create(&entry.lock)
            .and_then(|lock| {
                lock.try_lock()
                    .map(|()| lock)
                    .map_err(std::io::Error::other)
            })
            .map_err(|error| {
                format!(
                    "RUSTY_DEV_SESSION: could not lock `{}`: {error}",
                    entry.lock.display()
                )
            })?;
        locks.push(lock);
        let _ = fs::remove_file(&entry.stop);
        if keep {
            let _ = File::create(&entry.keep);
        }
        // Idle time counts from the start until the first use.
        let _ = File::create(&entry.activity);
        let project = super::absolute(project)?;
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut record = json!({
            "id": entry.directory.file_name().map(|name| name.to_string_lossy().into_owned()),
            "state": "starting",
            "project": project,
            "checkout": checkout,
            "pid": std::process::id(),
            "startedAt": started_at,
            "background": background,
            "persistenceRoot": persistence_root,
        });
        if let Some(label) = label {
            record["label"] = label.into();
        }
        if let Some(instance) = instance {
            record["instance"] = instance.into();
        }
        if background {
            record["log"] = json!(project_paths.log);
        }
        let session = Self {
            entry,
            project: project_paths,
            record: Mutex::new(record.as_object().cloned().unwrap_or_default()),
            retained: std::sync::atomic::AtomicBool::new(false),
            _locks: locks,
        };
        session.write();
        Ok(Arc::new(session))
    }

    /// The file the product host marks its use in.
    pub fn activity_file(&self) -> &Path {
        &self.entry.activity
    }

    /// Records use the host cannot see, such as a restage.
    pub fn touch_activity(&self) {
        let _ = File::create(&self.entry.activity)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()));
    }

    /// How long since the session was last used.
    pub fn idle(&self) -> Duration {
        self.entry.idle().unwrap_or_default()
    }

    /// Kept sessions never expire; `rusty dev keep` can set this at any time.
    pub fn kept(&self) -> bool {
        self.entry.keep.exists()
    }

    /// The runtime pack the session runs (its pair).
    pub fn uses_runtime(&self, pack: &Path) {
        self.update(|record| {
            record.insert("pair".into(), json!(pack));
        });
    }

    /// Ends the session for a reason its record keeps, so `rusty dev list
    /// --all` and callers can tell it from a crash or a stop.
    pub fn finish(&self, reason: &str) {
        self.retained
            .store(true, std::sync::atomic::Ordering::Release);
        let ended_at = epoch_seconds(std::time::SystemTime::now());
        self.update(|record| {
            record.insert("state".into(), reason.into());
            record.insert("endedAt".into(), ended_at.into());
        });
    }

    /// Records the idle limit, or why there is none.
    pub fn idle_limit(&self, limit: &Result<Duration, &'static str>) {
        self.update(|record| match limit {
            Ok(limit) => {
                record.insert("idleTimeoutMinutes".into(), (limit.as_secs() / 60).into());
            }
            Err(reason) => {
                record.insert("idleTimeoutMinutes".into(), Value::Null);
                record.insert("idleExpiry".into(), (*reason).into());
            }
        });
    }

    /// The `rusty-product-host` supervisor this session runs.
    pub fn host_started(&self, pid: u32) {
        self.update(|record| {
            record.insert("supervisorPid".into(), pid.into());
            record.remove("runtimePgid");
            record.remove("browserPgid");
        });
    }

    /// The process groups the supervisor reports for its runtime and
    /// headless browser, which a forced stop signals.
    fn processes(&self, report: &Value) {
        self.update(|record| {
            for (key, field) in [("runtime", "runtimePgid"), ("browser", "browserPgid")] {
                match report[key].as_u64() {
                    Some(pid) => record.insert(field.into(), pid.into()),
                    None => record.remove(field),
                };
            }
        });
    }

    pub fn stop_requested(&self) -> bool {
        self.entry.stop.exists() || self.project.stop.exists()
    }

    /// Copies a host's output to this supervisor's own and records where it
    /// serves once it says so.
    pub fn forward_host_output(self: &Arc<Self>, output: ChildStdout, runtime_instance_id: u64) {
        let session = Arc::clone(self);
        thread::spawn(move || {
            let mut output = BufReader::new(output);
            let mut line = Vec::new();
            while output
                .read_until(b'\n', &mut line)
                .is_ok_and(|read| read > 0)
            {
                let mut stdout = std::io::stdout().lock();
                let _ = stdout.write_all(&line).and_then(|()| stdout.flush());
                drop(stdout);
                let text = String::from_utf8_lossy(&line);
                if let Some((_, url)) = text.split_once(LISTENING) {
                    session.serving(url.trim(), runtime_instance_id);
                }
                if let Some(report) = text.trim_end().strip_prefix(PROCESSES) {
                    if let Ok(report) = serde_json::from_str::<Value>(report) {
                        session.processes(&report);
                    }
                }
                line.clear();
            }
        });
    }

    fn serving(&self, url: &str, runtime_instance_id: u64) {
        let port = url
            .rsplit_once(':')
            .and_then(|(_, port)| port.trim_end_matches('/').parse::<u16>().ok());
        self.update(|record| {
            record.insert("state".into(), "serving".into());
            record.insert("url".into(), url.into());
            record.insert("port".into(), port.into());
            record.insert("runtimeInstanceId".into(), runtime_instance_id.into());
        });
    }

    /// The host crashed past its restart budget and waits for a source edit.
    pub fn paused_fault(&self) {
        self.update(|record| {
            record.insert("state".into(), "paused-fault".into());
        });
    }

    fn update(&self, change: impl FnOnce(&mut Map<String, Value>)) {
        change(
            &mut self
                .record
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        self.write();
    }

    fn write(&self) {
        let record = Value::Object(
            self.record
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        for path in [&self.entry.record, &self.project.record] {
            let next = path.with_extension("json.next");
            let written =
                fs::write(&next, record.to_string()).and_then(|()| fs::rename(&next, path));
            if let Err(error) = written {
                super::diagnostic(
                    "session-record-failed",
                    json!({ "record": path, "error": error.to_string() }),
                );
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.project.record);
        let _ = fs::remove_file(&self.project.stop);
        if self.retained.load(std::sync::atomic::Ordering::Acquire) {
            // The record stays for `rusty dev list --all`; its lock is free
            // once this process exits.
            let _ = fs::remove_file(&self.entry.stop);
            let _ = fs::remove_file(&self.entry.keep);
        } else {
            // Windows keeps an open lock file; `rusty dev list` removes what is left.
            let _ = fs::remove_dir_all(&self.entry.directory);
        }
    }
}

/// Takes the project's lock: one session per project, or per instance.
fn claim_project(project: &Path, instance: Option<&str>) -> Result<(SessionPaths, File), String> {
    let paths = SessionPaths::for_project(project, instance)?;
    fs::create_dir_all(&paths.directory).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.directory.display()
        )
    })?;
    let lock = File::create(&paths.lock).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.lock.display()
        )
    })?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(format!(
                "RUSTY_DEV_SESSION_RUNNING: a session for `{}` is already running; `rusty dev list` shows it and `rusty dev stop <id>` ends it, or `--instance <name>` runs another",
                project.display()
            ))
        }
        Err(TryLockError::Error(error)) => {
            return Err(format!(
                "RUSTY_DEV_SESSION: could not lock `{}`: {error}",
                paths.lock.display()
            ))
        }
    }
    let _ = fs::remove_file(&paths.stop);
    Ok((paths, lock))
}

/// Starts `rusty dev <arguments> --session` in the background and returns
/// once it serves, printing its record.
pub fn start(
    project: &Path,
    instance: Option<&str>,
    arguments: &[OsString],
) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project, instance)?;
    if let Some(record) = paths.running()? {
        return Err(format!(
            "RUSTY_DEV_SESSION_RUNNING: a session for `{}` is already running: {record}",
            project.display()
        ));
    }
    fs::create_dir_all(&paths.directory).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.directory.display()
        )
    })?;
    let log = File::create(&paths.log).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.log.display()
        )
    })?;
    let log_for_errors = log
        .try_clone()
        .map_err(|error| format!("RUSTY_DEV_SESSION: {error}"))?;
    let executable =
        std::env::current_exe().map_err(|error| format!("RUSTY_DEV_SESSION: {error}"))?;
    let mut command = Command::new(executable);
    command
        .arg("dev")
        .args(arguments)
        .arg("--session")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_for_errors);
    detach(&mut command);
    let mut supervisor = command
        .spawn()
        .map_err(|error| format!("RUSTY_DEV_SESSION: could not start `rusty dev`: {error}"))?;
    loop {
        if let Some(status) = supervisor
            .try_wait()
            .map_err(|error| format!("RUSTY_DEV_SESSION: {error}"))?
        {
            return Err(format!(
                "RUSTY_DEV_SESSION_FAILED: `rusty dev` exited with {status} before serving; log `{}`:\n{}",
                paths.log.display(),
                paths.log_tail()
            ));
        }
        match paths.running()? {
            Some(record) if record["state"] == "serving" => {
                println!("{record}");
                return Ok(ExitCode::SUCCESS);
            }
            Some(record) if record["state"] == "paused-fault" => {
                let tail = paths.log_tail();
                stop_session(&paths)?;
                return Err(format!(
                    "RUSTY_DEV_SESSION_FAILED: the product host failed to start and the session was stopped; log `{}`:\n{tail}",
                    paths.log.display()
                ));
            }
            _ => thread::sleep(POLL),
        }
    }
}

pub fn stop(project: &Path, instance: Option<&str>) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project, instance)?;
    let stopped = stop_session(&paths)?;
    println!(
        "{}",
        json!({ "project": super::absolute(project)?, "stopped": stopped.is_some(), "session": stopped })
    );
    Ok(ExitCode::SUCCESS)
}

pub fn status(project: &Path, instance: Option<&str>) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project, instance)?;
    let session = paths.running()?;
    println!(
        "{}",
        json!({ "project": super::absolute(project)?, "running": session.is_some(), "session": session })
    );
    Ok(ExitCode::SUCCESS)
}

/// Asks a running supervisor to stop and waits until it has released its
/// lock, which it does after the product is disposed.
fn stop_session(paths: &SessionPaths) -> Result<Option<Value>, String> {
    let Some(record) = paths.running()? else {
        return Ok(None);
    };
    File::create(&paths.stop).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.stop.display()
        )
    })?;
    let deadline = Instant::now() + STOP_TIMEOUT;
    while paths.running()?.is_some() {
        if Instant::now() >= deadline {
            return Err(format!(
                "RUSTY_DEV_STOP_TIMEOUT: the session (supervisor pid {}) did not stop within {} seconds; log `{}`",
                record["pid"],
                STOP_TIMEOUT.as_secs(),
                paths.log.display()
            ));
        }
        thread::sleep(POLL);
    }
    Ok(Some(record))
}

/// `rusty dev list`: every running session on this machine; with `all`,
/// also those that ended for a reason in the last day.
pub fn list(machine_readable: bool, all: bool) -> Result<ExitCode, String> {
    let registry = Registry::machine()?;
    let scan = registry.scan();
    let mut sessions: Vec<Value> = scan.live.into_iter().map(|(_, record)| record).collect();
    if all {
        sessions.extend(scan.ended);
    }
    if machine_readable {
        println!("{}", Value::Array(sessions));
        return Ok(ExitCode::SUCCESS);
    }
    if sessions.is_empty() {
        println!(
            "no dev sessions are running (registry `{}`)",
            registry.root.display()
        );
        return Ok(ExitCode::SUCCESS);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    println!(
        "{:<34} {:<12} {:<28} {:>8} {:>13} {:<5} {:<24} PROJECT",
        "ID", "STATE", "URL", "UP", "IDLE/LIMIT", "KEEP", "LABEL"
    );
    for record in &sessions {
        let text = |key: &str| record[key].as_str().unwrap_or("-").to_owned();
        let up = now.saturating_sub(record["startedAt"].as_u64().unwrap_or(now));
        let idle = record["idleSeconds"]
            .as_u64()
            .map_or("-".to_owned(), duration_text);
        let limit = if record["keep"] == true {
            "kept".to_owned()
        } else {
            record["idleTimeoutMinutes"]
                .as_u64()
                .map_or("none".to_owned(), |minutes| duration_text(minutes * 60))
        };
        println!(
            "{:<34} {:<12} {:<28} {:>8} {:>13} {:<5} {:<24} {}",
            text("id"),
            text("state"),
            text("url"),
            duration_text(up),
            format!("{idle}/{limit}"),
            if record["keep"] == true { "yes" } else { "" },
            text("label"),
            text("project"),
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// `rusty dev stop <id|port>`: a graceful stop, then signals to the recorded
/// processes only, never a search by name.
pub fn stop_target(target: &str) -> Result<ExitCode, String> {
    let (paths, record) = Registry::machine()?.find(target)?;
    File::create(&paths.stop).map_err(|error| {
        format!(
            "RUSTY_DEV_SESSION: could not create `{}`: {error}",
            paths.stop.display()
        )
    })?;
    let stopped_within = |limit: Duration| {
        let deadline = Instant::now() + limit;
        while paths.alive() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(POLL);
        }
        true
    };
    let mut forced = false;
    if !stopped_within(GRACEFUL_STOP) {
        forced = true;
        // `rusty dev` stops its host and the host its runtime and browser.
        let targets = kill_targets(&paths.record());
        signal(&targets[..1.min(targets.len())], false);
        stopped_within(TERMINATE_GRACE);
        // Whatever is left of the recorded processes goes, whether or not
        // `rusty dev` itself has exited.
        let targets = kill_targets(&paths.record());
        signal(&targets, true);
        if !stopped_within(TERMINATE_GRACE) {
            return Err(format!(
                "RUSTY_DEV_STOP_TIMEOUT: session `{}` ({targets:?}) did not stop",
                record["id"].as_str().unwrap_or(target)
            ));
        }
    }
    let _ = fs::remove_dir_all(&paths.directory);
    println!(
        "{}",
        json!({ "stopped": true, "forced": forced, "session": record })
    );
    Ok(ExitCode::SUCCESS)
}

/// `rusty dev keep <id|port> [--off]`.
pub fn keep(target: &str, keep: bool) -> Result<ExitCode, String> {
    let (paths, _) = Registry::machine()?.find(target)?;
    let result = if keep {
        File::create(&paths.keep).map(drop)
    } else {
        fs::remove_file(&paths.keep).or_else(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Ok(())
            } else {
                Err(error)
            }
        })
    };
    result.map_err(|error| format!("RUSTY_DEV_SESSION: `{}`: {error}", paths.keep.display()))?;
    println!("{}", paths.record());
    Ok(ExitCode::SUCCESS)
}

/// `rusty dev prune`: removes the records of sessions that have ended.
pub fn prune() -> Result<ExitCode, String> {
    let (live, removed) = Registry::machine()?.live();
    println!("{}", json!({ "removed": removed, "running": live.len() }));
    Ok(ExitCode::SUCCESS)
}

/// Running and recently removed counts for `rusty status`.
pub fn registry_summary() -> Option<(PathBuf, usize)> {
    let registry = Registry::machine().ok()?;
    let running = registry.live().0.len();
    Some((registry.root, running))
}

fn duration_text(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m", seconds / 60),
        _ => format!("{}h{:02}m", seconds / 3600, seconds % 3600 / 60),
    }
}

/// A recorded process a forced stop signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KillTarget {
    Process(u64),
    /// A process group the supervisor reported (its runtime or browser).
    Group(u64),
}

/// The session's own processes, `rusty dev` first: its supervisor, then the
/// process groups of its runtime and headless browser.
fn kill_targets(record: &Value) -> Vec<KillTarget> {
    [
        ("pid", KillTarget::Process as fn(u64) -> KillTarget),
        ("supervisorPid", KillTarget::Process),
        ("runtimePgid", KillTarget::Group),
        ("browserPgid", KillTarget::Group),
    ]
    .into_iter()
    .filter_map(|(key, target)| record[key].as_u64().filter(|pid| *pid > 1).map(target))
    .collect()
}

/// Sends SIGTERM, or SIGKILL when `kill`, to `targets`.
fn signal(targets: &[KillTarget], kill: bool) {
    for target in targets {
        #[cfg(unix)]
        {
            let id = match target {
                KillTarget::Process(pid) => pid.to_string(),
                KillTarget::Group(pgid) => format!("-{pgid}"),
            };
            let _ = Command::new("kill")
                .args([if kill { "-KILL" } else { "-TERM" }, "--", &id])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        // Windows has no process groups to report; the tree kill covers
        // `rusty dev`'s descendants.
        #[cfg(windows)]
        if let KillTarget::Process(pid) = target {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T"])
                .args(kill.then_some("/F"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// Keeps terminal interrupts and the caller's process group away from the
/// background supervisor.
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn detach(command: &mut Command) {
    use std::os::windows::{io::AsRawHandle, process::CommandExt};
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
    unsafe extern "system" {
        fn SetHandleInformation(
            handle: std::os::windows::io::RawHandle,
            mask: u32,
            flags: u32,
        ) -> i32;
    }
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    // Windows hands a child every inheritable handle, so the supervisor would
    // otherwise hold this process's own output pipe open, and a caller that
    // reads `rusty dev start` to its end would wait until the session ends.
    for handle in [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ] {
        // SAFETY: the process's own standard handles, valid for its lifetime;
        // a failure only leaves the handle inheritable.
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Another test thread's fork can hold a released lock's file until its
    /// exec, so a check that a lock is free waits briefly.
    fn eventually(mut check: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !check() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
        true
    }

    fn registry(root: &Path) -> Registry {
        Registry {
            root: root.join("registry"),
        }
    }

    fn claim(
        registry: &Registry,
        project: &Path,
        background: bool,
    ) -> Result<Arc<Session>, String> {
        claim_instance(registry, project, background, None)
    }

    fn claim_instance(
        registry: &Registry,
        project: &Path,
        background: bool,
        instance: Option<&str>,
    ) -> Result<Arc<Session>, String> {
        Session::claim_in(
            registry,
            &Registration {
                project,
                persistence_root: Path::new("/persistence"),
                instance,
                checkout: project.parent().unwrap(),
                background,
                label: Some("test"),
                keep: false,
            },
        )
    }

    #[test]
    fn same_named_projects_have_their_own_sessions() {
        let root = std::env::temp_dir().join(format!("rusty-session-names-{}", std::process::id()));
        for directory in ["a", "b", "c"] {
            fs::create_dir_all(root.join(directory)).unwrap();
            fs::write(root.join(directory).join("Game.csproj"), "").unwrap();
        }
        fs::create_dir_all(root.join(".git")).unwrap();
        let registry = registry(&root);
        let (a, b) = (root.join("a/Game.csproj"), root.join("b/Game.csproj"));
        let session = claim(&registry, &a, true).unwrap();
        let (paths_a, paths_b) = (
            SessionPaths::for_project(&a, None).unwrap(),
            SessionPaths::for_project(&b, None).unwrap(),
        );
        assert_ne!(paths_a.directory, paths_b.directory);
        assert!(paths_a.running().unwrap().is_some());
        assert!(paths_b.running().unwrap().is_none());
        // Another spelling of a's project names a's session.
        let spelled = SessionPaths::for_project(&root.join("c/../a/./Game.csproj"), None).unwrap();
        assert_eq!(spelled.directory, paths_a.directory);
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_record_without_its_supervisor_is_cleared() {
        let root = std::env::temp_dir().join(format!("rusty-session-{}", std::process::id()));
        let project = root.join("Game.csproj");
        fs::create_dir_all(&root).unwrap();
        let registry = registry(&root);
        let session = claim(&registry, &project, true).unwrap();
        let paths = SessionPaths::for_project(&project, None).unwrap();
        assert_eq!(paths.running().unwrap().unwrap()["state"], "starting");
        assert!(claim(&registry, &project, true).is_err());
        session.serving("http://127.0.0.1:8787", 7);
        let record = paths.running().unwrap().unwrap();
        assert_eq!(
            (record["port"].clone(), record["runtimeInstanceId"].clone()),
            (json!(8787), json!(7))
        );

        // A supervisor that died without cleaning up leaves its record behind.
        // Another test thread's fork can hold the lock's file until its exec.
        let not_running = || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while paths.running().unwrap().is_some() {
                assert!(Instant::now() < deadline, "the lock stayed held");
                thread::sleep(Duration::from_millis(10));
            }
        };
        let record = fs::read(&paths.record).unwrap();
        drop(session);
        not_running();
        fs::write(&paths.record, record).unwrap();
        not_running();
        assert!(!paths.record.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_registry_lists_every_session_and_clears_ended_ones() {
        let root = std::env::temp_dir().join(format!("rusty-registry-{}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let registry = registry(&root);
        let (game, tool) = (root.join("Game.csproj"), root.join("Tool.csproj"));
        let foreground = claim(&registry, &game, false).unwrap();
        let background = claim(&registry, &tool, true).unwrap();
        foreground.serving("http://127.0.0.1:30301", 1);
        let (live, _) = registry.live();
        assert_eq!(live.len(), 2);
        let id = format!("game-{}", std::process::id());
        let (paths, record) = registry.find(&id).unwrap();
        assert_eq!(
            (record["label"].as_str(), record["background"].as_bool()),
            (Some("test"), Some(false))
        );
        assert_eq!(registry.find("30301").unwrap().1["id"], json!(id));
        assert!(registry.find("30302").is_err());

        // Idle time is the activity file's age; a touch resets it.
        let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
        File::options()
            .write(true)
            .open(foreground.activity_file())
            .unwrap()
            .set_modified(hour_ago)
            .unwrap();
        assert!(foreground.idle() >= Duration::from_secs(3599));
        assert!(registry.find(&id).unwrap().1["idleSeconds"].as_u64() >= Some(3599));
        foreground.touch_activity();
        assert!(foreground.idle() < Duration::from_secs(60));

        assert!(!foreground.kept());
        File::create(&paths.keep).unwrap();
        assert!(foreground.kept());
        assert_eq!(registry.find(&id).unwrap().1["keep"], true);
        File::create(&paths.stop).unwrap();
        assert!(foreground.stop_requested());

        // A session that ended without cleaning up leaves its directory; its
        // lock is free, so it is removed.
        let ended = registry.root.join("ended-1");
        fs::create_dir_all(&ended).unwrap();
        fs::write(ended.join("lock"), "").unwrap();
        fs::write(ended.join("session.json"), r#"{"id":"ended-1"}"#).unwrap();
        drop(foreground);
        let (live, removed) = registry.live();
        assert_eq!((live.len(), removed), (1, 1));
        assert!(!ended.exists());
        drop(background);
        assert!(registry.live().0.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn one_project_runs_one_session_unless_instances_name_others() {
        let root = std::env::temp_dir().join(format!("rusty-instances-{}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let registry = registry(&root);
        let game = root.join("Game.csproj");
        let first = claim(&registry, &game, false).unwrap();
        // A second foreground run of the project, or a background one, is refused.
        let refused = claim(&registry, &game, false).err().unwrap();
        assert!(
            refused.starts_with("RUSTY_DEV_SESSION_RUNNING"),
            "{refused}"
        );
        assert!(claim(&registry, &game, true).is_err());
        // The foreground run is found through its project, as `start` is.
        let paths = SessionPaths::for_project(&game, None).unwrap();
        assert!(paths.running().unwrap().is_some());
        // Named instances each have their own lock.
        let a = claim_instance(&registry, &game, false, Some("playtest-a")).unwrap();
        let b = claim_instance(&registry, &game, false, Some("playtest-b")).unwrap();
        assert!(claim_instance(&registry, &game, false, Some("playtest-a")).is_err());
        assert_eq!(registry.live().0.len(), 3);
        drop((first, a, b));
        assert!(eventually(|| claim(&registry, &game, false).is_ok()));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_record_names_what_a_forced_stop_signals() {
        let root = std::env::temp_dir().join(format!("rusty-record-{}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let registry = registry(&root);
        let session = claim(&registry, &root.join("Game.csproj"), true).unwrap();
        session.uses_runtime(Path::new("/cache/pairs/0.1.0-dev.x/runtime-pack"));
        session.host_started(4101);
        session.processes(&json!({ "runtime": 4102, "browser": 4103 }));
        session.serving("http://127.0.0.1:30301", 9);
        let (_, record) = registry.find("30301").unwrap();
        for key in [
            "id",
            "state",
            "project",
            "checkout",
            "pair",
            "pid",
            "supervisorPid",
            "runtimePgid",
            "browserPgid",
            "url",
            "port",
            "label",
            "startedAt",
            "lastActivity",
            "idleSeconds",
            "keep",
            "log",
            "persistenceRoot",
        ] {
            assert!(!record[key].is_null(), "the record lacks `{key}`: {record}");
        }
        assert_eq!(
            kill_targets(&record),
            [
                KillTarget::Process(u64::from(std::process::id())),
                KillTarget::Process(4101),
                KillTarget::Group(4102),
                KillTarget::Group(4103),
            ]
        );
        // A restarted runtime with no browser replaces the groups.
        session.processes(&json!({ "runtime": 4200, "browser": null }));
        let (_, record) = registry.find("30301").unwrap();
        assert!(record["browserPgid"].is_null());
        assert_eq!(kill_targets(&record).last(), Some(&KillTarget::Group(4200)));
        drop(session);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_session_that_ends_for_a_reason_keeps_its_record() {
        let root = std::env::temp_dir().join(format!("rusty-ended-{}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let registry = registry(&root);
        let expired = claim(&registry, &root.join("Game.csproj"), false).unwrap();
        let stopped = claim(&registry, &root.join("Tool.csproj"), false).unwrap();
        expired.finish("idle-expired");
        drop((expired, stopped));
        let mut scan = registry.scan();
        assert!(eventually(|| {
            scan = registry.scan();
            scan.live.is_empty()
        }));
        // The expired one is listed as ended; the plain stop leaves nothing,
        // as a crash would.
        assert_eq!(scan.ended.len(), 1);
        assert_eq!(scan.ended[0]["state"], "idle-expired");
        assert!(scan.ended[0]["endedAt"].as_u64().is_some());
        assert!(registry
            .find(scan.ended[0]["id"].as_str().unwrap())
            .is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
