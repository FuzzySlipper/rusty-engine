//! Outbound HTTPS transfers on Engine worker threads.
//!
//! A [`Transfer`] runs one GET on its own thread. Its body goes to memory or
//! to a file in a library directory; a file download is written to a hidden
//! partial file and renamed into place only once complete, so the library
//! never holds a file that looks finished but is not. Owners read a
//! [`TransferSnapshot`] whenever they choose; the worker never calls them.

use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock, PoisonError,
    },
    time::{Duration, Instant},
};

use ureq::{
    config::{Config, RedirectAuthHeaders},
    http::Uri,
    tls::{RootCerts, TlsConfig},
    unversioned::{
        resolver::DefaultResolver,
        transport::{
            Buffers, ConnectionDetails, Connector, DefaultConnector, NextTimeout, Transport,
        },
    },
    Agent,
};

/// A GET body larger than this fails instead of growing memory without
/// bound; large bodies belong in a download.
pub const MEMORY_BODY_LIMIT: u64 = 32 * 1024 * 1024;
/// A running transfer that receives nothing for this long fails as
/// interrupted. Without it a connection that silently drops (a laptop leaving
/// Wi-Fi) blocks for as long as the operating system keeps the socket.
#[cfg(not(test))]
pub const STALL_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(test)]
pub const STALL_TIMEOUT: Duration = Duration::from_secs(1);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);
const CHUNK: usize = 256 * 1024;
/// How long a blocked read waits before checking for a cancel or stall, so a
/// stopped transfer frees its thread and partial file within this time.
const POLL: Duration = Duration::from_millis(250);
const PARTIAL_SUFFIX: &str = ".partial";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    Running,
    Completed,
    Failed(Failure),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Connect,
    Interrupted,
    Status,
    BodyTooLarge,
    Storage,
    Protocol,
}

/// What a transfer has done so far. Cheap to clone: the body and headers are
/// shared.
#[derive(Debug, Clone)]
pub struct TransferSnapshot {
    pub state: TransferState,
    pub status: u16,
    pub received_bytes: u64,
    pub expected_bytes: u64,
    pub headers: Arc<[(String, String)]>,
    pub body: Arc<[u8]>,
    pub diagnostic: Arc<str>,
}

#[derive(Debug, Clone, Default)]
pub struct Headers {
    pub user_agent: String,
    pub accept: String,
    pub bearer_token: String,
    pub if_none_match: String,
}

#[derive(Debug, Clone)]
pub enum Destination {
    Memory,
    File {
        directory: PathBuf,
        file_name: String,
    },
}

#[derive(Debug)]
pub struct RequestError(pub String);

/// One shared HTTP agent: its connection pool is reused across transfers.
#[derive(Clone)]
pub struct HttpClient {
    config: Config,
}

impl Default for HttpClient {
    fn default() -> Self {
        let config = Config::builder()
            .http_status_as_error(false)
            .max_redirects(10)
            // GitHub release assets redirect to a CDN host, which must not
            // receive the API token.
            .redirect_auth_headers(RedirectAuthHeaders::SameHost)
            .user_agent(concat!("rusty-engine/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        Self { config }
    }
}

impl HttpClient {
    /// Starts a GET on a new worker thread.
    pub fn start(
        &self,
        url: &str,
        headers: Headers,
        destination: Destination,
    ) -> Result<Transfer, RequestError> {
        let uri: Uri = url
            .parse()
            .map_err(|cause| RequestError(format!("`{url}` is not a URL: {cause}")))?;
        if !matches!(uri.scheme_str(), Some("https" | "http")) || uri.host().is_none() {
            return Err(RequestError(format!(
                "`{url}` is not an absolute http or https URL"
            )));
        }
        let partial = match &destination {
            Destination::Memory => None,
            Destination::File {
                directory,
                file_name,
            } => {
                check_file_name(file_name)?;
                Some(Partial::claim(directory, file_name))
            }
        };
        let shared = Arc::new(Shared::new());
        // Each transfer's connections watch its own cancel flag.
        let agent = Agent::with_parts(
            self.config.clone(),
            DefaultConnector::new().chain(Interruptible {
                shared: Arc::clone(&shared),
            }),
            DefaultResolver::default(),
        );
        let worker = Worker {
            agent,
            uri,
            headers,
            shared: Arc::clone(&shared),
        };
        std::thread::Builder::new()
            .name("rusty-http".to_owned())
            .spawn(move || worker.run(destination, partial))
            .map_err(|cause| RequestError(format!("could not start a transfer thread: {cause}")))?;
        Ok(Transfer { shared })
    }
}

/// Wraps a transfer's connection so that a blocked read wakes every [`POLL`]
/// and gives up once the transfer is cancelled or has stalled.
struct Interruptible {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Interruptible {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Interruptible")
    }
}

impl Connector<Box<dyn Transport>> for Interruptible {
    type Out = InterruptibleTransport;

    fn connect(
        &self,
        _: &ConnectionDetails,
        chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(chained.map(|inner| InterruptibleTransport {
            inner,
            shared: Arc::clone(&self.shared),
        }))
    }
}

struct InterruptibleTransport {
    inner: Box<dyn Transport>,
    shared: Arc<Shared>,
}

impl std::fmt::Debug for InterruptibleTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("InterruptibleTransport")
            .field(&self.inner)
            .finish()
    }
}

impl Transport for InterruptibleTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        use ureq::unversioned::transport::time::Duration as UreqDuration;
        let mut remaining = timeout.after;
        loop {
            if self.shared.cancelled() || self.shared.stalled() {
                return Err(ureq::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "the transfer was stopped",
                )));
            }
            let wait = if remaining.is_not_happening() || *remaining > POLL {
                POLL
            } else {
                *remaining
            };
            match self.inner.await_input(NextTimeout {
                after: UreqDuration::from(wait),
                reason: timeout.reason,
            }) {
                Err(ureq::Error::Timeout(_)) if wait == POLL && *remaining != POLL => {
                    if !remaining.is_not_happening() {
                        remaining = UreqDuration::from(remaining.saturating_sub(POLL));
                    }
                }
                outcome => return outcome,
            }
        }
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }

    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}

/// The owner's side of one transfer. Dropping it cancels a running transfer.
pub struct Transfer {
    shared: Arc<Shared>,
}

impl Transfer {
    /// The transfer now. A running transfer that has stalled fails here.
    pub fn snapshot(&self) -> TransferSnapshot {
        let mut progress = self.shared.lock();
        if progress.state == TransferState::Running
            && progress.last_progress.elapsed() >= STALL_TIMEOUT
        {
            self.shared.cancel.store(true, Ordering::Relaxed);
            progress.finish(TransferState::Failed(Failure::Interrupted), stall_message());
        }
        progress.snapshot()
    }

    /// Stops a running transfer. Its partial file is removed by the worker.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
        let mut progress = self.shared.lock();
        if progress.state == TransferState::Running {
            progress.finish(TransferState::Cancelled, String::new());
        }
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Shared {
    cancel: AtomicBool,
    progress: Mutex<Progress>,
}

impl Shared {
    fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            progress: Mutex::new(Progress {
                state: TransferState::Running,
                status: 0,
                received_bytes: 0,
                expected_bytes: 0,
                headers: Arc::new([]),
                body: Arc::new([]),
                diagnostic: Arc::from(""),
                last_progress: Instant::now(),
            }),
        }
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    /// The worker's own view of a stall, so it stops even when no owner is
    /// looking.
    fn stalled(&self) -> bool {
        self.lock().last_progress.elapsed() >= STALL_TIMEOUT
    }
}

struct Progress {
    state: TransferState,
    status: u16,
    received_bytes: u64,
    expected_bytes: u64,
    headers: Arc<[(String, String)]>,
    body: Arc<[u8]>,
    diagnostic: Arc<str>,
    last_progress: Instant,
}

impl Progress {
    /// The first outcome is final: a later one from a worker that outlived a
    /// cancel or a stall is ignored.
    fn finish(&mut self, state: TransferState, diagnostic: String) {
        if self.state == TransferState::Running {
            self.state = state;
            self.diagnostic = Arc::from(diagnostic);
        }
    }
    fn snapshot(&self) -> TransferSnapshot {
        TransferSnapshot {
            state: self.state,
            status: self.status,
            received_bytes: self.received_bytes,
            expected_bytes: self.expected_bytes,
            headers: Arc::clone(&self.headers),
            body: Arc::clone(&self.body),
            diagnostic: Arc::clone(&self.diagnostic),
        }
    }
}

struct Worker {
    agent: Agent,
    uri: Uri,
    headers: Headers,
    shared: Arc<Shared>,
}

type Outcome = Result<(), (Failure, String)>;

impl Worker {
    fn run(self, destination: Destination, partial: Option<Partial>) {
        let outcome = self.transfer(destination, partial.as_ref());
        let mut progress = self.shared.lock();
        let installed = match outcome {
            // A download moves into place under this lock, so a cancel or
            // stall either precedes it (and no file appears) or follows it
            // (and changes nothing).
            Ok(()) if progress.state == TransferState::Running && !self.shared.cancelled() => {
                let installed = match &partial {
                    Some(partial) => fs::rename(&partial.path, &partial.target)
                        .map_err(|cause| (Failure::Storage, cause.to_string())),
                    None => Ok(()),
                };
                match installed {
                    Ok(()) => {
                        progress.finish(TransferState::Completed, String::new());
                        true
                    }
                    Err((failure, message)) => {
                        progress.finish(TransferState::Failed(failure), message);
                        false
                    }
                }
            }
            Ok(()) => {
                progress.finish(TransferState::Cancelled, String::new());
                false
            }
            Err(_) if progress.last_progress.elapsed() >= STALL_TIMEOUT => {
                progress.finish(TransferState::Failed(Failure::Interrupted), stall_message());
                false
            }
            Err((failure, message)) => {
                progress.finish(TransferState::Failed(failure), message);
                false
            }
        };
        drop(progress);
        if let Some(partial) = partial {
            partial.release(installed);
        }
    }

    fn transfer(&self, destination: Destination, partial: Option<&Partial>) -> Outcome {
        let mut request = self.agent.get(&self.uri);
        for (name, value) in [
            ("user-agent", &self.headers.user_agent),
            ("accept", &self.headers.accept),
            ("if-none-match", &self.headers.if_none_match),
        ] {
            if !value.is_empty() {
                request = request.header(name, value);
            }
        }
        if !self.headers.bearer_token.is_empty() {
            request = request.header(
                "authorization",
                format!("Bearer {}", self.headers.bearer_token),
            );
        }
        let response = request.call().map_err(|cause| {
            let failure = match cause {
                ureq::Error::TooManyRedirects
                | ureq::Error::Protocol(_)
                | ureq::Error::BadUri(_)
                | ureq::Error::Http(_) => Failure::Protocol,
                _ => Failure::Connect,
            };
            (failure, cause.to_string())
        })?;
        let status = response.status().as_u16();
        let headers: Arc<[(String, String)]> = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect();
        let expected = response.body().content_length().unwrap_or(0);
        {
            let mut progress = self.shared.lock();
            progress.status = status;
            progress.headers = headers;
            progress.expected_bytes = expected;
            progress.last_progress = Instant::now();
        }
        let mut reader = response.into_body().into_reader();
        match destination {
            Destination::Memory => {
                let mut body = Vec::new();
                self.copy(&mut reader, expected, |chunk| {
                    if body.len() as u64 + chunk.len() as u64 > MEMORY_BODY_LIMIT {
                        return Err((
                            Failure::BodyTooLarge,
                            format!(
                                "the body exceeds {MEMORY_BODY_LIMIT} bytes; download it to a library instead"
                            ),
                        ));
                    }
                    body.extend_from_slice(chunk);
                    Ok(())
                })?;
                self.shared.lock().body = Arc::from(body);
                Ok(())
            }
            Destination::File { .. } => {
                if !(200..300).contains(&status) {
                    return Err((Failure::Status, format!("the server answered {status}")));
                }
                let partial = partial.expect("a file destination claims a partial file");
                let storage = |cause: std::io::Error| (Failure::Storage, cause.to_string());
                let mut file = File::create(&partial.path).map_err(storage)?;
                self.copy(&mut reader, expected, |chunk| {
                    file.write_all(chunk).map_err(storage)
                })?;
                if !self.shared.cancelled() {
                    file.sync_all().map_err(storage)?;
                }
                Ok(())
            }
        }
    }

    /// Reads the body to its end, or until a cancel, handing each chunk on.
    fn copy(
        &self,
        reader: &mut impl Read,
        expected: u64,
        mut sink: impl FnMut(&[u8]) -> Outcome,
    ) -> Outcome {
        let mut buffer = vec![0; CHUNK];
        let mut received = 0u64;
        loop {
            if self.shared.cancelled() {
                return Ok(());
            }
            let read = reader
                .read(&mut buffer)
                .map_err(|cause| (Failure::Interrupted, cause.to_string()))?;
            if read == 0 {
                break;
            }
            sink(&buffer[..read])?;
            received += read as u64;
            let mut progress = self.shared.lock();
            progress.received_bytes = received;
            progress.last_progress = Instant::now();
        }
        if expected != 0 && received != expected {
            return Err((
                Failure::Interrupted,
                format!("the body ended after {received} of {expected} bytes"),
            ));
        }
        Ok(())
    }
}

fn stall_message() -> String {
    format!("no data arrived for {} s", STALL_TIMEOUT.as_secs())
}

/// A download's hidden partial file. Each has a unique name, so a worker that
/// outlives its transfer never touches a later download's file.
struct Partial {
    path: PathBuf,
    target: PathBuf,
}

/// Partial files this process is writing. Others are leftovers of a stopped
/// process and are removed when a download of the same name starts.
fn live_partials() -> &'static Mutex<HashSet<PathBuf>> {
    static LIVE: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    LIVE.get_or_init(Default::default)
}

impl Partial {
    fn claim(directory: &Path, file_name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let prefix = partial_prefix(file_name);
        let mut live = live_partials()
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&prefix)
                    && name.ends_with(PARTIAL_SUFFIX)
                    && !live.contains(&entry.path())
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let path = directory.join(format!(
            "{prefix}{}-{}{PARTIAL_SUFFIX}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        live.insert(path.clone());
        Self {
            path,
            target: directory.join(file_name),
        }
    }

    fn release(self, renamed: bool) {
        if !renamed {
            let _ = fs::remove_file(&self.path);
        }
        live_partials()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.path);
    }
}

fn partial_prefix(file_name: &str) -> String {
    format!(".{file_name}.")
}

fn is_partial(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(PARTIAL_SUFFIX)
}

/// A library file name is one path component the library lists: not a path,
/// and not a partial download's name.
pub fn check_file_name(file_name: &str) -> Result<(), RequestError> {
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || file_name.contains(['/', '\\', '\0'])
        || is_partial(file_name)
    {
        return Err(RequestError(format!(
            "`{file_name}` is not a library file name"
        )));
    }
    Ok(())
}

/// Creates the library directory if needed.
pub fn open_library(path: &Path) -> Result<(), RequestError> {
    fs::create_dir_all(path).map_err(|cause| {
        RequestError(format!(
            "could not open library {}: {cause}",
            path.display()
        ))
    })
}

/// The library's complete files, by name. Partial downloads are not listed.
pub fn library_files(directory: &Path) -> Result<Vec<(String, u64)>, RequestError> {
    let entries = fs::read_dir(directory).map_err(|cause| {
        RequestError(format!("could not list {}: {cause}", directory.display()))
    })?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() && !is_partial(&name) {
            files.push((name, metadata.len()));
        }
    }
    files.sort();
    Ok(files)
}

/// Removes one library file. False when it did not exist.
pub fn remove_library_file(directory: &Path, file_name: &str) -> Result<bool, RequestError> {
    check_file_name(file_name)?;
    match fs::remove_file(directory.join(file_name)) {
        Ok(()) => Ok(true),
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(RequestError(format!(
            "could not remove {file_name}: {cause}"
        ))),
    }
}

#[cfg(test)]
mod tests;
