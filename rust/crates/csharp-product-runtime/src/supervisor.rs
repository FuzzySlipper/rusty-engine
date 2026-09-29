//! Signal-owning supervisor for packaged product launches.
//!
//! This process binds the product listener and keeps terminal signals. The
//! runtime (the selected loader, Engine, product and browser I/O) is one child process in
//! its own process group that serves that listener directly; nothing is
//! relayed. The supervisor owns the `rusty dev` replacement contract, the one
//! automatic restart, the failure pause and headless browser launch.

use std::{
    env,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use product_dev_host::ProductDevLog;

use crate::{
    browser_url, headless_browser, install_termination_signal_hook, Arguments, ProductLoader,
};

/// Cold managed products can spend longer loading content than one callback.
const RUNTIME_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// A runtime that cannot finish product disposal is terminated.
const RUNTIME_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// The runtime prints this once its product is loaded, then waits for
/// [`SERVE_COMMAND`] on stdin before it accepts from the shared listener.
pub(crate) const RUNTIME_READY_LINE: &str = "RUSTY_RUNTIME ready";
pub(crate) const SERVE_COMMAND: &str = "serve";
/// Forwarded to a serving runtime after `rusty dev` restaged only UI or
/// bundle content: the runtime re-reads them without restarting the product.
pub(crate) const RELOAD_ASSETS_COMMAND: &str = "reload-assets";

/// `rusty dev` writes one command over stdin after each restage: a new
/// Product directory replaces the runtime, restaged UI or bundle content is
/// reloaded into it. EOF is a clean stop. Browser input cannot reach it.
#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum SupervisorCommand {
    ReplaceRuntime { product_directory: PathBuf },
    ReloadAssets,
}

pub(crate) fn run(args: Arguments) -> Result<(), String> {
    let termination = install_termination_signal_hook();
    let diagnostics = ProductDevLog::new(Default::default()).map_err(|error| error.to_string())?;
    let listener = TcpListener::bind(SocketAddr::from((args.bind_host(), args.port())))
        .map_err(|error| format!("DEV_HOST_BIND: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("DEV_HOST_ADDRESS: {error}"))?;
    let unavailable = Unavailable::start(
        listener
            .try_clone()
            .map_err(|error| format!("DEV_HOST_BIND: {error}"))?,
    )?;
    let mut launch = RuntimeLaunch {
        executable: env::current_exe().map_err(|error| error.to_string())?,
        listener_fd: listener.as_raw_fd(),
        product_directory: args
            .product_path
            .clone()
            .ok_or("DEV_HOST_SUPERVISOR: a packaged --product directory is required")?,
        loader: args.loader,
        next_runtime_instance_id: args
            .runtime_instance_id
            .ok_or("DEV_HOST_SUPERVISOR: the runtime incarnation was not allocated")?
            .value(),
        persistence_root: args.persistence_root.clone(),
        content_store_root: args.content_store_root.clone(),
        startup_timeout: (!args.debugger).then_some(RUNTIME_STARTUP_TIMEOUT),
    };
    if args.debugger {
        eprintln!(
            "RUSTY_HOST debugger: runtime startup deadline is disabled; source restaging still replaces the runtime"
        );
    }
    let mut runtime = Some(launch.start(&unavailable)?);
    // Until the first runtime serves, a failed start stops the supervisor.
    // Later ones, and any restaged Product, pause for a restage instead.
    let mut initial_start = true;
    let mut headless_browser = None;
    let commands = args.supervised.then(read_supervisor_commands);
    // One automatic recovery attempt is intentionally small. A repeated
    // failure pauses until source restaging supplies a new product; no
    // request is replayed in either case.
    let mut automatic_restart_used = false;
    let reason = loop {
        if termination.load(Ordering::Relaxed) {
            break "termination-signal";
        }
        if let Some(starting) = runtime.as_mut().filter(|runtime| !runtime.serving) {
            // A starting runtime is polled here, so signals, stdin EOF and
            // restages below still reach the supervisor while it loads.
            if let Err(error) = starting.poll_startup(&unavailable) {
                runtime = None;
                if initial_start {
                    return Err(error);
                }
                publish_supervisor_diagnostic(&diagnostics, "DEV_HOST_RUNTIME_START", &error);
                pause(
                    &diagnostics,
                    &unavailable,
                    "the runtime could not start; waiting for source restage",
                );
                automatic_restart_used = true;
            } else if starting.serving {
                initial_start = false;
                if args.headless && headless_browser.is_none() {
                    headless_browser = Some(headless_browser::HeadlessBrowser::launch(
                        &browser_url(address),
                    )?);
                }
            }
        } else if let Some(status) = runtime.as_mut().and_then(RuntimeProcess::exited) {
            runtime = None;
            let detail = format!("runtime exited unexpectedly ({status})");
            publish_supervisor_diagnostic(&diagnostics, "DEV_HOST_RUNTIME_EXIT", &detail);
            if !args.supervised {
                unavailable.stop();
                if let Some(browser) = headless_browser {
                    let _ = browser.shutdown();
                }
                return Err(format!(
                    "DEV_HOST_RUNTIME_EXIT: {detail}; stopping the host"
                ));
            }
            if automatic_restart_used {
                pause(
                    &diagnostics,
                    &unavailable,
                    "the runtime exhausted its one automatic restart; waiting for source restage",
                );
            } else {
                automatic_restart_used = true;
                runtime = start_or_pause(&mut launch, &unavailable, &diagnostics);
            }
        }
        let Some(commands) = &commands else {
            thread::sleep(POLL_INTERVAL);
            continue;
        };
        match commands.recv_timeout(POLL_INTERVAL) {
            Ok(Ok(SupervisorCommand::ReplaceRuntime { product_directory })) => {
                if !product_directory.is_absolute() {
                    publish_supervisor_diagnostic(
                        &diagnostics,
                        "DEV_HOST_SUPERVISOR_REPLACE",
                        "productDirectory must be absolute",
                    );
                    continue;
                }
                automatic_restart_used = false;
                initial_start = false;
                if let Some(previous) = runtime.take() {
                    if let Err(error) = previous.stop() {
                        publish_supervisor_diagnostic(
                            &diagnostics,
                            "DEV_HOST_RUNTIME_STOP",
                            &error,
                        );
                    }
                }
                launch.product_directory = product_directory;
                runtime = start_or_pause(&mut launch, &unavailable, &diagnostics);
                if runtime.is_none() {
                    automatic_restart_used = true;
                }
            }
            // A paused or restarting runtime loads the staged assets when it
            // starts, so there is nothing to forward.
            Ok(Ok(SupervisorCommand::ReloadAssets)) => {
                if let Some(Err(error)) = runtime.as_mut().map(RuntimeProcess::reload_assets) {
                    publish_supervisor_diagnostic(&diagnostics, "DEV_HOST_ASSET_RELOAD", &error);
                }
            }
            Ok(Err(error)) if error == SUPERVISOR_EOF => break "supervisor-stdin-closed",
            Ok(Err(error)) => {
                publish_supervisor_diagnostic(&diagnostics, "DEV_HOST_SUPERVISOR_CONTROL", &error);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break "supervisor-stdin-closed",
        }
    };
    unavailable.stop();
    let browser_shutdown = headless_browser
        .map(headless_browser::HeadlessBrowser::shutdown)
        .unwrap_or(Ok(()));
    let runtime_shutdown = runtime.map(RuntimeProcess::stop).unwrap_or(Ok(()));
    crate::print_line(&format!("RUSTY_HOST shutdown={{\"reason\":\"{reason}\"}}"));
    diagnostics.flush();
    browser_shutdown?;
    runtime_shutdown
}

fn start_or_pause(
    launch: &mut RuntimeLaunch,
    unavailable: &Unavailable,
    diagnostics: &ProductDevLog,
) -> Option<RuntimeProcess> {
    match launch.start(unavailable) {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            publish_supervisor_diagnostic(diagnostics, "DEV_HOST_RUNTIME_START", &error);
            pause(
                diagnostics,
                unavailable,
                "the runtime could not start; waiting for source restage",
            );
            None
        }
    }
}

fn pause(diagnostics: &ProductDevLog, unavailable: &Unavailable, reason: &str) {
    publish_supervisor_diagnostic(diagnostics, "DEV_HOST_RUNTIME_PAUSED", reason);
    unavailable.answer(reason);
}

fn publish_supervisor_diagnostic(diagnostics: &ProductDevLog, code: &str, message: &str) {
    eprintln!("RUSTY_HOST {code}: {message}");
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

const SUPERVISOR_EOF: &str = "DEV_HOST_SUPERVISOR_EOF: supervisor stdin closed";

fn read_supervisor_commands() -> mpsc::Receiver<Result<SupervisorCommand, String>> {
    let (commands, received) = mpsc::sync_channel(4);
    thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let command = read_supervisor_frame(&mut input);
            let stop = command.is_err();
            if commands.send(command).is_err() || stop {
                return;
            }
        }
    });
    received
}

/// One command: a little-endian `u32` length followed by that many JSON bytes.
fn read_supervisor_frame(input: &mut impl Read) -> Result<SupervisorCommand, String> {
    let mut prefix = [0_u8; 4];
    if let Err(error) = input.read_exact(&mut prefix) {
        return Err(if error.kind() == std::io::ErrorKind::UnexpectedEof {
            SUPERVISOR_EOF.to_owned()
        } else {
            format!("DEV_HOST_SUPERVISOR_READ: {error}")
        });
    }
    let mut bytes = vec![0_u8; u32::from_le_bytes(prefix) as usize];
    input
        .read_exact(&mut bytes)
        .map_err(|_| SUPERVISOR_EOF.to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "DEV_HOST_SUPERVISOR_DECODE: command is not a closed envelope".to_owned())
}

struct RuntimeLaunch {
    executable: PathBuf,
    listener_fd: i32,
    product_directory: PathBuf,
    loader: ProductLoader,
    next_runtime_instance_id: u64,
    persistence_root: Option<PathBuf>,
    content_store_root: Option<PathBuf>,
    startup_timeout: Option<Duration>,
}

impl RuntimeLaunch {
    /// Spawns one runtime incarnation. Requests are answered with 503 while
    /// it loads; [`RuntimeProcess::poll_startup`] hands it the listener.
    fn start(&mut self, unavailable: &Unavailable) -> Result<RuntimeProcess, String> {
        unavailable.answer("the runtime is starting");
        let runtime_instance_id = self.next_runtime_instance_id;
        self.next_runtime_instance_id = runtime_instance_id.saturating_add(1).max(1);
        let arguments = self.runtime_arguments(runtime_instance_id)?;
        let mut runtime = RuntimeProcess::spawn(&self.executable, &arguments, self.listener_fd)?;
        runtime.startup_deadline = self.startup_timeout.map(|timeout| Instant::now() + timeout);
        Ok(runtime)
    }

    fn runtime_arguments(&self, runtime_instance_id: u64) -> Result<Vec<String>, String> {
        let product = self
            .product_directory
            .to_str()
            .ok_or("DEV_HOST_SUPERVISOR: the Product directory path must be UTF-8")?;
        let mut arguments = vec![
            "--product".to_owned(),
            product.to_owned(),
            "--loader".to_owned(),
            self.loader.identifier().to_owned(),
            "--runtime-instance-id".to_owned(),
            runtime_instance_id.to_string(),
            "--serve-listener-fd".to_owned(),
            self.listener_fd.to_string(),
        ];
        for (flag, root) in [
            ("--persistence-root", &self.persistence_root),
            ("--content-store-root", &self.content_store_root),
        ] {
            if let Some(root) = root {
                arguments.push(flag.to_owned());
                arguments.push(path_argument(root)?);
            }
        }
        Ok(arguments)
    }
}

fn path_argument(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("DEV_HOST_SUPERVISOR: `{}` is not UTF-8", path.display()))
}

/// One runtime incarnation. Its stdin is the control pipe: one `serve`
/// line, any number of `reload-assets` lines, then EOF as the clean-stop
/// request.
struct RuntimeProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    ready: mpsc::Receiver<()>,
    startup_deadline: Option<Instant>,
    serving: bool,
    reload_after_serve: bool,
}

impl RuntimeProcess {
    fn spawn(executable: &Path, arguments: &[String], listener_fd: i32) -> Result<Self, String> {
        let mut command = Command::new(executable);
        command
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Terminal Ctrl+C and foreground-group signals reach only this
            // supervisor; CoreCLR stops through its stdin instead.
            .process_group(0);
        // SAFETY: only the async-signal-safe fcntl runs between fork and
        // exec. It clears close-on-exec so the runtime inherits the listener.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(listener_fd, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("DEV_HOST_RUNTIME_SPAWN: {error}"))?;
        let stdout = child.stdout.take().expect("piped runtime stdout");
        let (ready, ready_rx) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line == RUNTIME_READY_LINE {
                    let _ = ready.send(());
                } else {
                    crate::print_line(&line);
                }
            }
        });
        Ok(Self {
            stdin: child.stdin.take(),
            child,
            ready: ready_rx,
            startup_deadline: None,
            serving: false,
            reload_after_serve: false,
        })
    }

    /// Checks a starting runtime once. When it reports ready, the supervisor
    /// stops answering before the runtime accepts, so the two processes never
    /// accept from the listener at once.
    fn poll_startup(&mut self, unavailable: &Unavailable) -> Result<(), String> {
        if self.ready.try_recv().is_ok() {
            unavailable.suspend();
            self.serve()?;
            self.serving = true;
            if self.reload_after_serve {
                self.reload_assets()?;
            }
            return Ok(());
        }
        if let Some(status) = self.exited() {
            return Err(format!(
                "DEV_HOST_RUNTIME_EXIT: the runtime exited during startup ({status})"
            ));
        }
        if self
            .startup_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.kill();
            return Err(
                "DEV_HOST_RUNTIME_STARTUP_TIMEOUT: the runtime did not load its product within 30 seconds"
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn serve(&mut self) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or("DEV_HOST_RUNTIME_SERVE: runtime stdin is closed")?;
        writeln!(stdin, "{SERVE_COMMAND}")
            .and_then(|_| stdin.flush())
            .map_err(|error| format!("DEV_HOST_RUNTIME_SERVE: {error}"))
    }

    /// A starting runtime reads only `serve` first, and may already have read
    /// the previous assets; it gets the reload right after `serve`.
    fn reload_assets(&mut self) -> Result<(), String> {
        if !self.serving {
            self.reload_after_serve = true;
            return Ok(());
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or("DEV_HOST_ASSET_RELOAD: runtime stdin is closed")?;
        writeln!(stdin, "{RELOAD_ASSETS_COMMAND}")
            .and_then(|_| stdin.flush())
            .map_err(|error| format!("DEV_HOST_ASSET_RELOAD: {error}"))
    }

    fn exited(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Closes stdin, the clean-stop request, and waits for product disposal.
    fn stop(mut self) -> Result<(), String> {
        drop(self.stdin.take());
        let deadline = Instant::now() + RUNTIME_SHUTDOWN_TIMEOUT;
        loop {
            if let Some(status) = self.exited() {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "DEV_HOST_RUNTIME_EXIT: the runtime exited with {status}"
                    ))
                };
            }
            if Instant::now() >= deadline {
                self.kill();
                return Err(
                    "DEV_HOST_RUNTIME_SHUTDOWN_TIMEOUT: the runtime did not dispose within 10 seconds"
                        .to_owned(),
                );
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn kill(&mut self) {
        if self.exited().is_some() {
            return;
        }
        // The runtime leads its own process group.
        // SAFETY: killpg only sends a signal to that group.
        unsafe {
            libc::killpg(self.child.id() as libc::pid_t, libc::SIGKILL);
        }
        let _ = self.child.wait();
    }
}

impl Drop for RuntimeProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Answers requests while no runtime is serving the shared listener, so
/// browsers get a prompt 503 instead of waiting in the listen backlog.
struct Unavailable {
    reason: Arc<Mutex<String>>,
    answering: Arc<AtomicBool>,
    /// Held across one poll and accept; `suspend` takes it to know that no
    /// accept is in progress. The answer path never waits for it.
    accepting: Arc<Mutex<()>>,
    stopped: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Unavailable {
    fn start(listener: TcpListener) -> Result<Self, String> {
        let reason = Arc::new(Mutex::new(String::new()));
        let answering = Arc::new(AtomicBool::new(false));
        let accepting = Arc::new(Mutex::new(()));
        let stopped = Arc::new(AtomicBool::new(false));
        let thread = {
            let reason = Arc::clone(&reason);
            let answering = Arc::clone(&answering);
            let accepting = Arc::clone(&accepting);
            let stopped = Arc::clone(&stopped);
            thread::Builder::new()
                .name("rusty-supervisor-unavailable".to_owned())
                .spawn(move || {
                    while !stopped.load(Ordering::Acquire) {
                        if !answering.load(Ordering::Acquire) {
                            thread::sleep(POLL_INTERVAL);
                            continue;
                        }
                        let _accepting =
                            accepting.lock().unwrap_or_else(|error| error.into_inner());
                        if !answering.load(Ordering::Acquire) {
                            continue;
                        }
                        let mut poll = libc::pollfd {
                            fd: listener.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        // SAFETY: one valid pollfd for the duration of the call.
                        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
                        if ready > 0 && poll.revents & libc::POLLIN != 0 {
                            if let Ok((stream, _)) = listener.accept() {
                                let current = reason
                                    .lock()
                                    .unwrap_or_else(|error| error.into_inner())
                                    .clone();
                                thread::spawn(move || answer_unavailable(stream, &current));
                            }
                        }
                    }
                })
                .map_err(|error| format!("DEV_HOST_SUPERVISOR_THREAD: {error}"))?
        };
        Ok(Self {
            reason,
            answering,
            accepting,
            stopped,
            thread: Some(thread),
        })
    }

    fn answer(&self, reason: &str) {
        *self
            .reason
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = reason.to_owned();
        self.answering.store(true, Ordering::Release);
    }

    /// Returns once no accept is in progress, so a runtime can take over.
    /// With `answering` cleared the thread does not take the lock again, so
    /// this waits at most one poll interval.
    fn suspend(&self) {
        self.answering.store(false, Ordering::Release);
        drop(
            self.accepting
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        );
    }

    fn stop(&self) {
        self.suspend();
        self.stopped.store(true, Ordering::Release);
    }
}

impl Drop for Unavailable {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn answer_unavailable(mut stream: TcpStream, reason: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let mut head = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") && head.len() < 16 * 1024 {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => head.extend_from_slice(&buffer[..count]),
        }
    }
    let navigation = String::from_utf8_lossy(&head)
        .lines()
        .any(|line| line.to_ascii_lowercase().starts_with("accept:") && line.contains("text/html"));
    let (content_type, body) = if navigation {
        (
            "text/html; charset=utf-8",
            format!(
                "<!doctype html><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"1\"><title>Runtime unavailable</title><p>{}</p>",
                escape_html(reason)
            ),
        )
    } else {
        (
            "application/json",
            serde_json::json!({
                "accepted": false,
                "error": { "code": "DEV_HOST_RUNTIME_UNAVAILABLE", "diagnostic": reason },
            })
            .to_string(),
        )
    };
    let response = format!(
        "HTTP/1.1 503 Service Unavailable\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nRetry-After: 1\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_frames_decode_replacement_and_report_eof() {
        let mut frame = Vec::new();
        for body in [
            br#"{"kind":"replace-runtime","productDirectory":"/tmp/product"}"#.as_slice(),
            br#"{"kind":"reload-assets"}"#.as_slice(),
        ] {
            frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
            frame.extend_from_slice(body);
        }
        let mut input = frame.as_slice();
        let Ok(SupervisorCommand::ReplaceRuntime { product_directory }) =
            read_supervisor_frame(&mut input)
        else {
            panic!("expected a replacement");
        };
        assert_eq!(product_directory, PathBuf::from("/tmp/product"));
        assert!(matches!(
            read_supervisor_frame(&mut input),
            Ok(SupervisorCommand::ReloadAssets)
        ));
        assert_eq!(
            read_supervisor_frame(&mut input).unwrap_err(),
            SUPERVISOR_EOF
        );
    }

    #[test]
    fn runtime_launch_forwards_the_selected_loader() {
        for loader in [ProductLoader::NativeAot, ProductLoader::CoreClr] {
            let launch = RuntimeLaunch {
                executable: PathBuf::from("/runtime/rusty-product-host"),
                listener_fd: 3,
                product_directory: PathBuf::from("/tmp/product"),
                loader,
                next_runtime_instance_id: 7,
                persistence_root: None,
                content_store_root: None,
                startup_timeout: None,
            };
            let arguments = launch.runtime_arguments(7).unwrap();
            let selected = arguments
                .iter()
                .position(|argument| argument == "--loader")
                .map(|index| arguments[index + 1].as_str());
            assert_eq!(selected, Some(loader.identifier()));
        }
    }

    #[test]
    fn answering_and_suspending_do_not_wait_behind_the_responder() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable = Unavailable::start(listener).unwrap();
        unavailable.answer("the runtime is starting");
        // The responder now repeatedly polls while holding its accept lock.
        thread::sleep(Duration::from_millis(250));
        for _ in 0..20 {
            let started = Instant::now();
            unavailable.answer("the runtime could not start");
            assert!(started.elapsed() < Duration::from_millis(20));
        }
        let started = Instant::now();
        unavailable.suspend();
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn unavailable_answers_until_suspended() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let unavailable = Unavailable::start(listener.try_clone().unwrap()).unwrap();
        unavailable.answer("the runtime is starting");
        let request = |accept: &str| {
            let mut stream = TcpStream::connect(address).unwrap();
            write!(
                stream,
                "GET / HTTP/1.1\r\nHost: {address}\r\nAccept: {accept}\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        };
        let page = request("text/html");
        assert!(page.starts_with("HTTP/1.1 503"));
        assert!(page.contains("http-equiv=\"refresh\""));
        let api = request("application/json");
        assert!(api.contains("DEV_HOST_RUNTIME_UNAVAILABLE"));
        assert!(api.contains("the runtime is starting"));
        unavailable.suspend();
        // A suspended supervisor leaves connections to the runtime's accept.
        let _pending = TcpStream::connect(address).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let accepted = loop {
            match listener.accept() {
                Ok(_) => break true,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Err(_) => break false,
            }
        };
        assert!(accepted, "the suspended supervisor took the connection");
    }
}
