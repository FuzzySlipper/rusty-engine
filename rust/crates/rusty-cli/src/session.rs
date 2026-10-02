//! `rusty dev start|stop|status`: one background `rusty dev` per product
//! project, found through files under the repository's `.runtime/dev/<project
//! file>/` instead of process names.
//!
//! The supervisor holds an exclusive lock on `lock` for its whole life, so a
//! record whose lock is free belongs to a supervisor that died and is cleared.
//! `stop` asks the supervisor to shut down by creating the `stop` file, which
//! its watch loop sees; it then closes the host's stdin as Ctrl+C would, so the
//! product is disposed.

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

pub struct SessionPaths {
    directory: PathBuf,
    record: PathBuf,
    lock: PathBuf,
    stop: PathBuf,
    log: PathBuf,
}

impl SessionPaths {
    pub fn for_project(project: &Path) -> Result<Self, String> {
        let project = super::absolute(project)?;
        let name = project.file_name().ok_or_else(|| {
            format!(
                "RUSTY_DEV_PROJECT: `{}` names no project file",
                project.display()
            )
        })?;
        let directory = super::development_runtime_root(&project)?
            .join("dev")
            .join(name);
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

/// The supervisor's side: the lock it holds and the record it keeps current.
pub struct Session {
    paths: SessionPaths,
    record: Mutex<Map<String, Value>>,
    _lock: File,
}

impl Session {
    pub fn claim(project: &Path, persistence_root: &Path) -> Result<Arc<Self>, String> {
        let paths = SessionPaths::for_project(project)?;
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
                    "RUSTY_DEV_SESSION_RUNNING: a session for `{}` is already running; `rusty dev stop --project {}` ends it",
                    project.display(),
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
        let record = json!({
            "state": "starting",
            "project": super::absolute(project)?,
            "pid": std::process::id(),
            "persistenceRoot": persistence_root,
            "log": paths.log,
        });
        let session = Self {
            paths,
            record: Mutex::new(record.as_object().cloned().unwrap_or_default()),
            _lock: lock,
        };
        session.write();
        Ok(Arc::new(session))
    }

    pub fn stop_requested(&self) -> bool {
        self.paths.stop.exists()
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
        let next = self.paths.record.with_extension("json.next");
        let written = fs::write(&next, record.to_string())
            .and_then(|()| fs::rename(&next, &self.paths.record));
        if let Err(error) = written {
            super::diagnostic(
                "session-record-failed",
                json!({ "record": self.paths.record, "error": error.to_string() }),
            );
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.paths.record);
        let _ = fs::remove_file(&self.paths.stop);
    }
}

/// Starts `rusty dev <arguments> --session` in the background and returns
/// once it serves, printing its record.
pub fn start(project: &Path, arguments: &[OsString]) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project)?;
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

pub fn stop(project: &Path) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project)?;
    let stopped = stop_session(&paths)?;
    println!(
        "{}",
        json!({ "project": super::absolute(project)?, "stopped": stopped.is_some(), "session": stopped })
    );
    Ok(ExitCode::SUCCESS)
}

pub fn status(project: &Path) -> Result<ExitCode, String> {
    let paths = SessionPaths::for_project(project)?;
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

/// Keeps terminal interrupts and the caller's process group away from the
/// background supervisor.
#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_without_its_supervisor_is_cleared() {
        let root = std::env::temp_dir().join(format!("rusty-session-{}", std::process::id()));
        let project = root.join("Game.csproj");
        fs::create_dir_all(&root).unwrap();
        let session = Session::claim(&project, &root.join(".runtime/persistence")).unwrap();
        let paths = SessionPaths::for_project(&project).unwrap();
        assert_eq!(paths.running().unwrap().unwrap()["state"], "starting");
        assert!(Session::claim(&project, &root).is_err());
        session.serving("http://127.0.0.1:8787", 7);
        let record = paths.running().unwrap().unwrap();
        assert_eq!(
            (record["port"].clone(), record["runtimeInstanceId"].clone()),
            (json!(8787), json!(7))
        );

        // A supervisor that died without cleaning up leaves its record behind.
        let record = fs::read(&paths.record).unwrap();
        drop(session);
        assert!(paths.running().unwrap().is_none());
        fs::write(&paths.record, record).unwrap();
        assert!(paths.running().unwrap().is_none());
        assert!(!paths.record.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
