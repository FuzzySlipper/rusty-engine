//! One run of a tool executable as a child process. Its standard output and
//! error are read line by line into a short window of recent lines; a waiter
//! thread records how it ended.

use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, PoisonError,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// The recent output lines a run keeps: enough for a progress line and a
/// failure's message, without growing for a tool that prints a lot.
pub const OUTPUT_LINES: usize = 64;
/// How often the waiter looks for an exit or a cancel.
const POLL: Duration = Duration::from_millis(50);
/// A console tool started from a window process opens no console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRunState {
    Running,
    /// The tool ended by itself with this exit code (-1 when a signal ended
    /// it).
    Exited(i32),
    Cancelled,
    /// The executable could not be started; the output says why.
    FailedToStart,
}

#[derive(Debug, Clone)]
pub struct ToolRunSnapshot {
    pub state: ToolRunState,
    /// The recent output lines, oldest first, each ending in a newline.
    pub output: Arc<str>,
    /// Every line the tool has written so far, including those no longer
    /// in `output`.
    pub lines: u64,
}

struct Progress {
    state: ToolRunState,
    recent: VecDeque<String>,
    lines: u64,
    /// `recent` joined, rebuilt only after a new line.
    text: Option<Arc<str>>,
}

impl Progress {
    fn push(&mut self, line: String) {
        if self.recent.len() == OUTPUT_LINES {
            self.recent.pop_front();
        }
        self.recent.push_back(line);
        self.lines += 1;
        self.text = None;
    }
}

struct Shared {
    progress: Mutex<Progress>,
    cancel: AtomicBool,
}

impl Shared {
    fn progress(&self) -> std::sync::MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A running or finished tool. Dropping it cancels a run still going.
pub struct ToolRun {
    shared: Arc<Shared>,
}

impl ToolRun {
    /// Starts `executable` with `arguments` in `working_directory`, with no
    /// standard input. A tool that cannot start is a run in `FailedToStart`.
    pub fn start(executable: &Path, arguments: &[String], working_directory: &Path) -> Self {
        let shared = Arc::new(Shared {
            progress: Mutex::new(Progress {
                state: ToolRunState::Running,
                recent: VecDeque::new(),
                lines: 0,
                text: None,
            }),
            cancel: AtomicBool::new(false),
        });
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);
        match command.spawn() {
            Ok(mut child) => {
                let readers = [
                    child.stdout.take().map(|out| read_lines(out, &shared)),
                    child.stderr.take().map(|err| read_lines(err, &shared)),
                ];
                let waiter = Arc::clone(&shared);
                thread::Builder::new()
                    .name("rusty-tool-run".to_owned())
                    .spawn(move || wait(child, readers, &waiter))
                    .expect("the tool run's waiter thread starts");
            }
            Err(error) => {
                let mut progress = shared.progress();
                progress.push(format!(
                    "{} could not start in {}: {error}\n",
                    executable.display(),
                    working_directory.display()
                ));
                progress.state = ToolRunState::FailedToStart;
            }
        }
        Self { shared }
    }

    pub fn snapshot(&self) -> ToolRunSnapshot {
        let mut progress = self.shared.progress();
        let output = match &progress.text {
            Some(text) => Arc::clone(text),
            None => {
                let text: Arc<str> = progress
                    .recent
                    .iter()
                    .map(String::as_str)
                    .collect::<String>()
                    .into();
                progress.text = Some(Arc::clone(&text));
                text
            }
        };
        ToolRunSnapshot {
            state: progress.state,
            output,
            lines: progress.lines,
        }
    }

    /// Asks a running tool to end. The run keeps reading `Running` until the
    /// tool has stopped, then `Cancelled`; a tool that exited first keeps its
    /// exit code.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Release);
    }
}

impl Drop for ToolRun {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn read_lines(stream: impl Read + Send + 'static, shared: &Arc<Shared>) -> JoinHandle<()> {
    let shared = Arc::clone(shared);
    thread::Builder::new()
        .name("rusty-tool-output".to_owned())
        .spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match reader.read_until(b'\n', &mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&bytes);
                        let line = line.trim_end_matches(['\n', '\r']);
                        shared.progress().push(format!("{line}\n"));
                    }
                }
            }
        })
        .expect("the tool run's output thread starts")
}

/// Owns the child until it has ended, so the run reads finished only once the
/// tool can no longer write anything.
fn wait(mut child: Child, readers: [Option<JoinHandle<()>>; 2], shared: &Shared) {
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if shared.cancel.load(Ordering::Acquire) => {
                let _ = child.kill();
                let ended = child.wait();
                let mut progress = shared.progress();
                if let Err(error) = ended {
                    progress.push(format!("waiting for the cancelled tool failed: {error}\n"));
                }
                progress.state = ToolRunState::Cancelled;
                return;
            }
            Ok(None) => thread::sleep(POLL),
            Err(error) => {
                shared
                    .progress()
                    .push(format!("waiting for the tool failed: {error}\n"));
                break -1;
            }
        }
    };
    // The last lines arrive before the run reads as finished.
    for reader in readers.into_iter().flatten() {
        let _ = reader.join();
    }
    shared.progress().state = ToolRunState::Exited(status);
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Instant;

    fn settle(run: &ToolRun) -> ToolRunSnapshot {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let snapshot = run.snapshot();
            if snapshot.state != ToolRunState::Running || Instant::now() > deadline {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn shell(script: &str, directory: &Path) -> ToolRun {
        ToolRun::start(
            Path::new("/bin/sh"),
            &["-c".to_owned(), script.to_owned()],
            directory,
        )
    }

    #[test]
    fn a_finished_run_keeps_its_exit_code_and_every_line_it_wrote() {
        let directory = tempfile::tempdir().unwrap();
        let run = shell("pwd; echo out; echo err >&2; exit 3", directory.path());
        let snapshot = settle(&run);
        assert_eq!(snapshot.state, ToolRunState::Exited(3));
        assert_eq!(snapshot.lines, 3);
        // Standard output and error are read apart, so only each stream's own
        // lines keep their order.
        let lines: Vec<&str> = snapshot.output.lines().collect();
        let stdout: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| *line != "err")
            .collect();
        let pwd = directory.path().canonicalize().unwrap();
        assert_eq!(stdout, [pwd.to_str().unwrap(), "out"]);
        assert!(lines.contains(&"err"));
    }

    #[test]
    fn output_keeps_only_the_recent_lines_and_counts_all_of_them() {
        let directory = tempfile::tempdir().unwrap();
        let run = shell(
            "i=0; while [ $i -lt 100 ]; do echo line$i; i=$((i+1)); done",
            directory.path(),
        );
        let snapshot = settle(&run);
        assert_eq!(snapshot.state, ToolRunState::Exited(0));
        assert_eq!(snapshot.lines, 100);
        let lines: Vec<&str> = snapshot.output.lines().collect();
        assert_eq!(lines.len(), OUTPUT_LINES);
        assert_eq!(lines.first(), Some(&"line36"));
        assert_eq!(lines.last(), Some(&"line99"));
    }

    #[test]
    /// A run reads `Cancelled` only once the tool has stopped: from then on
    /// it writes nothing more.
    fn a_cancelled_run_reads_cancelled_only_once_the_tool_has_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let ticks = directory.path().join("ticks");
        // The shell itself appends (a builtin), so killing it stops every write.
        let run = shell(
            "echo started; while :; do echo tick >> ticks; done",
            directory.path(),
        );
        while run.snapshot().lines == 0 {
            thread::sleep(Duration::from_millis(10));
        }
        run.cancel();
        let snapshot = settle(&run);
        assert_eq!(snapshot.state, ToolRunState::Cancelled);
        let written = std::fs::metadata(&ticks).unwrap().len();
        assert!(written > 0);
        thread::sleep(POLL * 4);
        assert_eq!(std::fs::metadata(&ticks).unwrap().len(), written);
    }

    #[test]
    fn a_tool_that_exited_before_its_cancel_keeps_its_exit_code() {
        let directory = tempfile::tempdir().unwrap();
        let run = shell("exit 5", directory.path());
        let snapshot = settle(&run);
        run.cancel();
        assert_eq!(snapshot.state, ToolRunState::Exited(5));
        assert_eq!(run.snapshot().state, ToolRunState::Exited(5));
    }

    #[test]
    fn a_missing_executable_fails_to_start_and_says_why() {
        let directory = tempfile::tempdir().unwrap();
        let run = ToolRun::start(&directory.path().join("absent"), &[], directory.path());
        let snapshot = run.snapshot();
        assert_eq!(snapshot.state, ToolRunState::FailedToStart);
        assert!(
            snapshot.output.contains("could not start"),
            "{}",
            snapshot.output
        );
    }
}
