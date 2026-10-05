//! Small-party sessions: one host and a handful of guests over Iroh (QUIC
//! dialled by endpoint key, relayed when no direct path exists).
//!
//! The host owns the session. Each guest holds one reliable, ordered stream
//! to the host; the host relays nothing between guests except the roster and
//! the chat transcript. Network tasks run on a [`SessionRuntime`] and only
//! record observations; the owner reads them with [`Session::drain`].

mod invitation;
mod wire;

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicU32, AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard, PoisonError,
    },
    time::Duration,
};

use iroh::{
    endpoint::{presets, Connection, QuicTransportConfig, SendStream},
    Endpoint, RelayMode, SecretKey,
};
use tokio::sync::mpsc;

pub use invitation::Invitation;
pub use wire::MAX_FRAME;
use wire::{read_frame, write_frame, ChatEntry, Frame, RosterEntry, PROTOCOL};

const ALPN: &[u8] = b"rusty-engine/session/1";
/// A peer that sends nothing (keep-alives included) for this long is gone.
/// Iroh's 30 s default left players waiting half a minute to learn that a
/// friend's game had closed.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// Reaching a relay, a host, or a guest's first frame.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const HELLO_TIMEOUT: Duration = Duration::from_secs(15);
/// Bytes queued for one member beyond which it is disconnected as too slow:
/// a stalled peer would otherwise grow the sender's memory without bound.
pub const SLOW_MEMBER_BYTES: usize = 32 * 1024 * 1024;
/// Received bytes awaiting the owner beyond which the sending peer is
/// disconnected: a peer cannot grow this process's memory without bound
/// while the product is paused.
pub const PENDING_EVENT_BYTES: usize = 64 * 1024 * 1024;
/// Chat lines longer than this are refused; it bounds the transcript.
pub const MAX_CHAT_BYTES: usize = 4 * 1024;
/// Transcript lines the host keeps and sends to joining members.
pub const CHAT_LINES: usize = 1000;

pub const HOST_MEMBER: u32 = 1;

/// The tokio runtime every session of one owner runs on.
pub struct SessionRuntime {
    runtime: Option<tokio::runtime::Runtime>,
}

impl SessionRuntime {
    pub fn new() -> Result<Self, String> {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("rusty-session")
            .enable_all()
            .build()
            .map(|runtime| Self {
                runtime: Some(runtime),
            })
            .map_err(|cause| format!("could not start the session runtime: {cause}"))
    }

    fn handle(&self) -> tokio::runtime::Handle {
        self.runtime
            .as_ref()
            .expect("the runtime lives until drop")
            .handle()
            .clone()
    }
}

impl Drop for SessionRuntime {
    fn drop(&mut self) {
        // Leaves its sessions' goodbyes a moment to go out.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(Duration::from_secs(1));
        }
    }
}

/// A local identity: the key a peer's member number follows across rejoins.
#[derive(Clone)]
pub struct Identity(SecretKey);

impl Identity {
    pub fn ephemeral() -> Self {
        Self(SecretKey::generate())
    }

    /// The identity stored at `path`, created there on first use.
    pub fn stored(path: &Path) -> Result<Self, String> {
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(bytes) = <[u8; 32]>::try_from(bytes.as_slice()) {
                return Ok(Self(SecretKey::from_bytes(&bytes)));
            }
        }
        let key = SecretKey::generate();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|cause| format!("could not store the identity: {cause}"))?;
        }
        std::fs::write(path, key.to_bytes())
            .map_err(|cause| format!("could not store the identity: {cause}"))?;
        Ok(Self(key))
    }

    pub fn key(&self) -> String {
        self.0.public().to_string()
    }
}

/// Which relay a host uses. Guests use the one in the invitation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Relay {
    /// Direct connections only: a LAN, or hosts reachable without NAT help.
    None,
    /// number 0's public relays: free, rate-limited, for development.
    N0,
    Url(iroh::RelayUrl),
}

impl Relay {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "" => Ok(Self::None),
            "n0" => Ok(Self::N0),
            url => url
                .parse()
                .map(Self::Url)
                .map_err(|cause| format!("`{url}` is not a relay URL: {cause}")),
        }
    }
    fn mode(&self, token: &str) -> RelayMode {
        match self {
            Self::None => RelayMode::Disabled,
            Self::N0 => RelayMode::Default,
            Self::Url(url) => relay_mode([url.clone()], token),
        }
    }
}

pub struct HostConfig {
    pub identity: Identity,
    pub application: String,
    pub relay: Relay,
    /// The access token a self-hosted relay requires, if any. Guests receive
    /// it in the invitation.
    pub relay_token: String,
    /// Never use a direct path: peers see only the relay's address.
    pub relay_only: bool,
}

pub struct JoinConfig {
    pub identity: Identity,
    pub application: String,
    pub invitation: Invitation,
    pub relay_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Open,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// This side left.
    Left,
    /// The host ended the session.
    HostClosed,
    /// The connection to the host broke or timed out.
    HostLost,
    /// The host refused the join.
    Refused,
    /// The host or relay could not be reached.
    Unreachable,
    /// This side could not start (binding, identity, relay).
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveReason {
    Left,
    Lost,
    TooSlow,
    Flooded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    None,
    Direct,
    Relay,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Opened,
    Joined {
        member: u32,
    },
    Rejoined {
        member: u32,
    },
    Left {
        member: u32,
        reason: LeaveReason,
    },
    /// From `member`. On the host, `seen` is the last host sequence the guest
    /// had received; on a guest, `sequence` is the host's.
    Message {
        member: u32,
        sequence: u64,
        seen: u64,
        payload: Vec<u8>,
    },
    /// A fresh view from the host (guests only).
    View {
        sequence: u64,
        payload: Vec<u8>,
    },
    Ended {
        reason: EndReason,
    },
}

#[derive(Debug, Clone)]
pub struct MemberView {
    pub member: u32,
    pub key: String,
    pub is_host: bool,
    pub is_local: bool,
    pub connected: bool,
    pub awaiting_view: bool,
    pub path: PathKind,
    pub rtt: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatState {
    Pending,
    Delivered,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ChatLine {
    /// The host's transcript sequence; zero while pending or failed.
    pub sequence: u64,
    pub local_id: u64,
    pub member: u32,
    pub state: ChatState,
    pub text: String,
}

/// Everything observed since the previous drain.
pub struct Drained {
    pub state: State,
    pub role: Role,
    pub end_reason: Option<EndReason>,
    pub diagnostic: String,
    pub local_member: u32,
    pub invitation: String,
    pub host_sequence: u64,
    pub chat_revision: u64,
    pub members: Vec<MemberView>,
    pub chat: Vec<ChatLine>,
    pub events: Vec<Event>,
}

/// A send's outcome. `queued` is false when the recipient is not connected:
/// nothing was sent and nothing will be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendReceipt {
    pub sequence: u64,
    pub recipients: u32,
}

struct Writer {
    tx: mpsc::UnboundedSender<Vec<u8>>,
    queued: Arc<AtomicUsize>,
    /// Why this side closed the connection, if it did.
    closed: Arc<AtomicU32>,
    connection: Connection,
    task: tokio::task::JoinHandle<()>,
}

impl Writer {
    /// Queues a frame, or disconnects a member whose queue is full. False
    /// when nothing was queued.
    fn send(&self, frame: &Frame) -> bool {
        let encoded = frame.encode();
        let len = encoded.len();
        if self.queued.fetch_add(len, Ordering::Relaxed) + len > SLOW_MEMBER_BYTES {
            self.close(CLOSE_SLOW);
            return false;
        }
        self.tx.send(encoded).is_ok()
    }

    fn close(&self, code: u32) {
        self.closed.store(code, Ordering::Relaxed);
        self.connection.close(code.into(), b"");
    }
}

const OPEN: u32 = 0;
const CLOSE_SLOW: u32 = 1;
const CLOSE_FLOODED: u32 = 2;
const CLOSE_REPLACED: u32 = 3;
const CLOSE_REFUSED: u32 = 4;

struct Member {
    key: String,
    is_host: bool,
    connected: bool,
    awaiting_view: bool,
    writer: Option<Writer>,
}

struct Shared {
    state: State,
    role: Role,
    end_reason: Option<EndReason>,
    diagnostic: String,
    local_member: u32,
    invitation: String,
    host_sequence: u64,
    members: BTreeMap<u32, Member>,
    events: Vec<Event>,
    pending_bytes: usize,
    chat: Vec<ChatLine>,
    chat_revision: u64,
    next_chat: u64,
    endpoint: Option<Endpoint>,
}

impl Shared {
    /// Ends the session, handing back what is left to close: the endpoint and
    /// the writers still sending.
    fn end(
        &mut self,
        reason: EndReason,
        diagnostic: impl Into<String>,
    ) -> (Option<Endpoint>, Vec<Writer>) {
        if self.state == State::Ended {
            return (None, Vec::new());
        }
        self.state = State::Ended;
        self.end_reason = Some(reason);
        self.diagnostic = diagnostic.into();
        self.events.push(Event::Ended { reason });
        for line in &mut self.chat {
            if line.state == ChatState::Pending {
                line.state = ChatState::Failed;
                self.chat_revision += 1;
            }
        }
        let local = self.local_member;
        let mut writers = Vec::new();
        for (&member, entry) in &mut self.members {
            entry.connected = member == local;
            writers.extend(entry.writer.take());
        }
        (self.endpoint.take(), writers)
    }

    /// Ends the session and closes its endpoint once the writers have sent
    /// what they hold, for at most a second.
    fn end_and_close(&mut self, reason: EndReason, diagnostic: impl Into<String>) {
        let (endpoint, writers) = self.end(reason, diagnostic);
        tokio::spawn(async move {
            for writer in writers {
                drop(writer.tx);
                let _ = tokio::time::timeout(Duration::from_secs(1), writer.task).await;
            }
            if let Some(endpoint) = endpoint {
                endpoint.close().await;
            }
        });
    }

    /// Records a received event, or reports that its sender has sent more
    /// than the owner has drained.
    fn receive(&mut self, event: Event, bytes: usize) -> bool {
        self.pending_bytes += bytes;
        self.events.push(event);
        self.pending_bytes <= PENDING_EVENT_BYTES
    }

    fn roster(&self) -> Vec<RosterEntry> {
        self.members
            .iter()
            .map(|(&member, entry)| RosterEntry {
                member,
                key: entry.key.clone(),
                is_host: entry.is_host,
                connected: entry.connected,
            })
            .collect()
    }

    fn broadcast_roster(&self) {
        let frame = Frame::Roster(self.roster());
        for member in self.members.values() {
            if let Some(writer) = &member.writer {
                writer.send(&frame);
            }
        }
    }

    fn transcript(&self) -> Vec<ChatEntry> {
        self.chat
            .iter()
            .filter(|line| line.state == ChatState::Delivered)
            .map(|line| ChatEntry {
                sequence: line.sequence,
                member: line.member,
                local_id: line.local_id,
                text: line.text.clone(),
            })
            .collect()
    }

    /// Host: appends a line to the transcript and sends it to every guest.
    fn host_chat(&mut self, member: u32, local_id: u64, text: String) {
        let sequence = self.chat.last().map_or(1, |line| line.sequence + 1);
        let line = ChatEntry {
            sequence,
            member,
            local_id,
            text,
        };
        for entry in self.members.values() {
            if let Some(writer) = &entry.writer {
                writer.send(&Frame::ChatLine(line.clone()));
            }
        }
        self.chat.push(ChatLine {
            sequence,
            local_id,
            member,
            state: ChatState::Delivered,
            text: line.text,
        });
        if self.chat.len() > CHAT_LINES {
            self.chat.remove(0);
        }
        self.chat_revision += 1;
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One side of a session. Dropping it leaves.
pub struct Session {
    shared: Arc<Mutex<Shared>>,
    runtime: tokio::runtime::Handle,
}

impl Session {
    pub fn host(runtime: &SessionRuntime, config: HostConfig) -> Self {
        let session = Self::new(runtime, Role::Host);
        let shared = Arc::clone(&session.shared);
        session.runtime.spawn(async move {
            if let Err((reason, diagnostic)) = run_host(Arc::clone(&shared), config).await {
                lock(&shared).end_and_close(reason, diagnostic);
            }
        });
        session
    }

    pub fn join(runtime: &SessionRuntime, config: JoinConfig) -> Self {
        let session = Self::new(runtime, Role::Guest);
        let shared = Arc::clone(&session.shared);
        session.runtime.spawn(async move {
            let (reason, diagnostic) = run_guest(Arc::clone(&shared), config).await;
            lock(&shared).end_and_close(reason, diagnostic);
        });
        session
    }

    fn new(runtime: &SessionRuntime, role: Role) -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                state: State::Starting,
                role,
                end_reason: None,
                diagnostic: String::new(),
                local_member: 0,
                invitation: String::new(),
                host_sequence: 0,
                members: BTreeMap::new(),
                events: Vec::new(),
                pending_bytes: 0,
                chat: Vec::new(),
                chat_revision: 0,
                next_chat: 1,
                endpoint: None,
            })),
            runtime: runtime.handle(),
        }
    }

    pub fn drain(&self) -> Drained {
        let mut shared = lock(&self.shared);
        shared.pending_bytes = 0;
        let events = std::mem::take(&mut shared.events);
        let members = shared
            .members
            .iter()
            .map(|(&member, entry)| {
                let (path, rtt) = entry
                    .writer
                    .as_ref()
                    .map_or((PathKind::None, Duration::ZERO), |writer| {
                        selected_path(&writer.connection)
                    });
                MemberView {
                    member,
                    key: entry.key.clone(),
                    is_host: entry.is_host,
                    is_local: member == shared.local_member,
                    connected: entry.connected,
                    awaiting_view: entry.awaiting_view,
                    path,
                    rtt,
                }
            })
            .collect();
        Drained {
            state: shared.state,
            role: shared.role,
            end_reason: shared.end_reason,
            diagnostic: shared.diagnostic.clone(),
            local_member: shared.local_member,
            invitation: shared.invitation.clone(),
            host_sequence: shared.host_sequence,
            chat_revision: shared.chat_revision,
            members,
            chat: shared.chat.clone(),
            events,
        }
    }

    /// Host: one member. Guest: the host (`member` must be [`HOST_MEMBER`]).
    pub fn send(&self, member: u32, payload: &[u8]) -> Result<SendReceipt, String> {
        check_payload(payload)?;
        let mut shared = lock(&self.shared);
        let sequence = match shared.role {
            Role::Host => {
                shared.host_sequence += 1;
                shared.host_sequence
            }
            Role::Guest => {
                if member != HOST_MEMBER {
                    return Err("a guest sends only to the host".to_owned());
                }
                0
            }
        };
        let frame = Frame::Message {
            sequence,
            seen: if shared.role == Role::Guest {
                shared.host_sequence
            } else {
                0
            },
            payload: payload.to_vec(),
        };
        let delivered = match shared.members.get(&member) {
            Some(entry) if member != shared.local_member => entry
                .writer
                .as_ref()
                .is_some_and(|writer| writer.send(&frame)),
            Some(_) => return Err("a member does not send to itself".to_owned()),
            None => return Err(format!("no member {member} in this session")),
        };
        Ok(SendReceipt {
            sequence,
            recipients: u32::from(delivered),
        })
    }

    /// Host only: every connected member that has received its view.
    pub fn broadcast(&self, payload: &[u8]) -> Result<SendReceipt, String> {
        check_payload(payload)?;
        let mut shared = lock(&self.shared);
        if shared.role != Role::Host {
            return Err("only the host broadcasts".to_owned());
        }
        shared.host_sequence += 1;
        let sequence = shared.host_sequence;
        let frame = Frame::Message {
            sequence,
            seen: 0,
            payload: payload.to_vec(),
        };
        let recipients = shared
            .members
            .values()
            .filter(|entry| !entry.awaiting_view)
            .filter_map(|entry| entry.writer.as_ref())
            .filter(|writer| writer.send(&frame))
            .count() as u32;
        Ok(SendReceipt {
            sequence,
            recipients,
        })
    }

    /// Host only: a fresh view for one member. Broadcasts reach a joined or
    /// rejoined member only after its view, so the view is the first thing it
    /// needs and everything after it follows in order.
    pub fn send_view(&self, member: u32, payload: &[u8]) -> Result<SendReceipt, String> {
        check_payload(payload)?;
        let mut shared = lock(&self.shared);
        if shared.role != Role::Host {
            return Err("only the host sends views".to_owned());
        }
        if member == HOST_MEMBER {
            return Err("the host does not send itself a view".to_owned());
        }
        shared.host_sequence += 1;
        let sequence = shared.host_sequence;
        let Some(entry) = shared.members.get_mut(&member) else {
            return Err(format!("no member {member} in this session"));
        };
        let frame = Frame::View {
            sequence,
            payload: payload.to_vec(),
        };
        let delivered = entry
            .writer
            .as_ref()
            .is_some_and(|writer| writer.send(&frame));
        if delivered {
            entry.awaiting_view = false;
        }
        Ok(SendReceipt {
            sequence,
            recipients: u32::from(delivered),
        })
    }

    /// Adds a chat line. A guest's line is pending until the host's
    /// transcript carries it, and fails if the connection ends first.
    pub fn send_chat(&self, text: &str) -> Result<u64, String> {
        if text.is_empty() || text.len() > MAX_CHAT_BYTES {
            return Err(format!(
                "a chat line has 1 to {MAX_CHAT_BYTES} bytes, not {}",
                text.len()
            ));
        }
        let mut shared = lock(&self.shared);
        let local_id = shared.next_chat;
        shared.next_chat += 1;
        let local_member = shared.local_member;
        match shared.role {
            Role::Host if shared.state == State::Open => {
                shared.host_chat(local_member, local_id, text.to_owned());
            }
            _ => {
                let sent = shared.state == State::Open
                    && shared
                        .members
                        .get(&HOST_MEMBER)
                        .and_then(|host| host.writer.as_ref())
                        .is_some_and(|writer| {
                            writer.send(&Frame::Chat {
                                local_id,
                                text: text.to_owned(),
                            })
                        });
                shared.chat.push(ChatLine {
                    sequence: 0,
                    local_id,
                    member: local_member,
                    state: if sent {
                        ChatState::Pending
                    } else {
                        ChatState::Failed
                    },
                    text: text.to_owned(),
                });
                shared.chat_revision += 1;
            }
        }
        Ok(local_id)
    }

    /// Leaves now: peers are told, and the session ends.
    pub fn leave(&self) {
        let mut shared = lock(&self.shared);
        if shared.state == State::Ended {
            return;
        }
        for member in shared.members.values() {
            if let Some(writer) = &member.writer {
                writer.send(&Frame::Bye);
            }
        }
        let _guard = self.runtime.enter();
        shared.end_and_close(EndReason::Left, "");
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Relays by URL, presenting `token` when one is given.
pub(crate) fn relay_mode(urls: impl IntoIterator<Item = iroh::RelayUrl>, token: &str) -> RelayMode {
    let map: iroh::RelayMap = urls.into_iter().collect();
    if map.is_empty() {
        RelayMode::Disabled
    } else if token.is_empty() {
        RelayMode::Custom(map)
    } else {
        RelayMode::Custom(map.with_auth_token(token))
    }
}

/// Waits for `work` for up to [`CONNECT_TIMEOUT`], failing at once when the
/// relay refuses this endpoint (a wrong access token), not at the timeout.
async fn until_relayed<T>(
    endpoint: &Endpoint,
    work: impl std::future::Future<Output = T>,
) -> Result<T, String> {
    use iroh::Watcher as _;
    let mut status = endpoint.home_relay_status();
    let refused = async {
        loop {
            if let Some(reason) = status.get().iter().find_map(|relay| {
                relay
                    .auth_denied_reason()
                    .map(|reason| (relay.url().to_string(), reason.to_owned()))
            }) {
                return reason;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::select! {
        done = tokio::time::timeout(CONNECT_TIMEOUT, work) => done.map_err(|_| {
            let last = status
                .get()
                .iter()
                .find_map(|relay| relay.last_error().map(|error| format!(" ({}: {error})", relay.url())));
            format!("the host or relay could not be reached{}", last.unwrap_or_default())
        }),
        (url, reason) = refused => Err(format!("the relay {url} refused access: {reason}")),
    }
}

fn check_payload(payload: &[u8]) -> Result<(), String> {
    if payload.len() > MAX_FRAME {
        return Err(format!(
            "a message carries at most {MAX_FRAME} bytes, not {}",
            payload.len()
        ));
    }
    Ok(())
}

fn selected_path(connection: &Connection) -> (PathKind, Duration) {
    for path in connection.paths().iter() {
        if path.is_selected() {
            let kind = if path.is_relay() {
                PathKind::Relay
            } else {
                PathKind::Direct
            };
            return (kind, path.stats().rtt);
        }
    }
    (PathKind::None, Duration::ZERO)
}

async fn bind(
    identity: &Identity,
    relay: &RelayMode,
    relay_only: bool,
    accept: bool,
) -> Result<Endpoint, String> {
    let transport = QuicTransportConfig::builder()
        .max_idle_timeout(Some(
            IDLE_TIMEOUT
                .try_into()
                .expect("the idle timeout fits QUIC's range"),
        ))
        .build();
    let mut builder = Endpoint::builder(presets::Minimal)
        .secret_key(identity.0.clone())
        .relay_mode(relay.clone())
        .transport_config(transport);
    if accept {
        builder = builder.alpns(vec![ALPN.to_vec()]);
    }
    if relay_only {
        builder = builder.clear_ip_transports();
    }
    builder
        .bind()
        .await
        .map_err(|cause| format!("could not open a network endpoint: {cause}"))
}

/// Starts the writer task for one stream; frames go out in queue order.
fn writer(mut send: SendStream, connection: Connection, closed: Arc<AtomicU32>) -> Writer {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let queued = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queued);
    let task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            let len = frame.len();
            let bye = Frame::decode(&frame[4..]) == Some(Frame::Bye);
            if write_frame(&mut send, &frame).await.is_err() {
                return;
            }
            counter.fetch_sub(len, Ordering::Relaxed);
            if bye {
                let _ = send.finish();
                let _ = send.stopped().await;
                return;
            }
        }
    });
    Writer {
        tx,
        queued,
        closed,
        connection,
        task,
    }
}

async fn run_host(
    shared: Arc<Mutex<Shared>>,
    config: HostConfig,
) -> Result<(), (EndReason, String)> {
    let endpoint = bind(
        &config.identity,
        &config.relay.mode(&config.relay_token),
        config.relay_only,
        true,
    )
    .await
    .map_err(|cause| (EndReason::Failed, cause))?;
    if config.relay != Relay::None {
        if let Err(reason) = until_relayed(&endpoint, endpoint.online()).await {
            endpoint.close().await;
            return Err((EndReason::Unreachable, reason));
        }
    }
    let secret = invitation::secret();
    let invitation = Invitation::new(
        &endpoint,
        config.relay != Relay::None,
        &config.relay_token,
        &secret,
        &config.application,
    );
    let left_already = lock(&shared).state == State::Ended;
    if left_already {
        endpoint.close().await;
        return Ok(());
    }
    {
        let mut shared = lock(&shared);
        shared.state = State::Open;
        shared.local_member = HOST_MEMBER;
        shared.invitation = invitation.encode();
        shared.members.insert(
            HOST_MEMBER,
            Member {
                key: config.identity.key(),
                is_host: true,
                connected: true,
                awaiting_view: false,
                writer: None,
            },
        );
        shared.endpoint = Some(endpoint.clone());
        shared.events.push(Event::Opened);
    }
    let application = Arc::new(config.application);
    let secret = Arc::new(secret);
    while let Some(incoming) = endpoint.accept().await {
        let shared = Arc::clone(&shared);
        let application = Arc::clone(&application);
        let secret = Arc::clone(&secret);
        tokio::spawn(async move {
            if let Ok(connecting) = incoming.accept() {
                if let Ok(connection) = connecting.await {
                    serve_guest(shared, connection, &application, &secret).await;
                }
            }
        });
    }
    Ok(())
}

async fn serve_guest(
    shared: Arc<Mutex<Shared>>,
    connection: Connection,
    application: &str,
    secret: &str,
) {
    let Ok(Ok((send, mut recv))) =
        tokio::time::timeout(HELLO_TIMEOUT, connection.accept_bi()).await
    else {
        return;
    };
    let hello = tokio::time::timeout(HELLO_TIMEOUT, read_frame(&mut recv)).await;
    let refusal = match hello {
        Ok(Ok(Some(Frame::Hello {
            protocol,
            application: theirs,
            secret: offered,
        }))) => {
            if protocol != PROTOCOL {
                Some(format!(
                    "the host runs session protocol {PROTOCOL}, the guest {protocol}"
                ))
            } else if theirs != application {
                Some(format!(
                    "the host runs `{application}`, the guest `{theirs}`"
                ))
            } else if offered != secret {
                Some("the invitation is not for this session".to_owned())
            } else {
                None
            }
        }
        _ => return,
    };
    let closed = Arc::new(AtomicU32::new(OPEN));
    let writer = writer(send, connection.clone(), Arc::clone(&closed));
    if let Some(reason) = refusal {
        writer.send(&Frame::Refused { reason });
        writer.send(&Frame::Bye);
        drop(writer.tx);
        let _ = tokio::time::timeout(Duration::from_secs(1), writer.task).await;
        connection.close(CLOSE_REFUSED.into(), b"refused");
        return;
    }
    let key = connection.remote_id().to_string();
    let member = {
        let mut shared = lock(&shared);
        if shared.state != State::Open {
            return;
        }
        let existing = shared
            .members
            .iter()
            .find(|(_, entry)| entry.key == key)
            .map(|(&member, _)| member);
        let member = existing.unwrap_or_else(|| {
            shared
                .members
                .keys()
                .next_back()
                .copied()
                .unwrap_or(HOST_MEMBER)
                + 1
        });
        if let Some(old) = shared
            .members
            .get_mut(&member)
            .and_then(|entry| entry.writer.take())
        {
            // A rejoin before the old connection timed out replaces it.
            old.close(CLOSE_REPLACED);
        }
        let welcome = Frame::Welcome {
            member,
            roster: Vec::new(),
            chat: shared.transcript(),
        };
        writer.send(&welcome);
        shared.members.insert(
            member,
            Member {
                key: key.clone(),
                is_host: false,
                connected: true,
                awaiting_view: true,
                writer: Some(writer),
            },
        );
        shared.events.push(if existing.is_some() {
            Event::Rejoined { member }
        } else {
            Event::Joined { member }
        });
        shared.broadcast_roster();
        member
    };
    let reason = loop {
        match read_frame(&mut recv).await {
            Ok(Some(Frame::Message { seen, payload, .. })) => {
                let bytes = payload.len();
                let within = lock(&shared).receive(
                    Event::Message {
                        member,
                        sequence: 0,
                        seen,
                        payload,
                    },
                    bytes,
                );
                if !within {
                    closed.store(CLOSE_FLOODED, Ordering::Relaxed);
                    connection.close(CLOSE_FLOODED.into(), b"");
                    break LeaveReason::Flooded;
                }
            }
            Ok(Some(Frame::Chat { local_id, text })) => {
                if text.is_empty() || text.len() > MAX_CHAT_BYTES {
                    continue;
                }
                let mut shared = lock(&shared);
                shared.pending_bytes += text.len();
                shared.host_chat(member, local_id, text);
            }
            Ok(Some(Frame::Bye)) | Ok(None) => break LeaveReason::Left,
            Ok(Some(_)) => {}
            Err(_) => {
                break match (closed.load(Ordering::Relaxed), connection.close_reason()) {
                    (CLOSE_SLOW, _) => LeaveReason::TooSlow,
                    // The guest's game closed its endpoint.
                    (OPEN, Some(iroh::endpoint::ConnectionError::ApplicationClosed(_))) => {
                        LeaveReason::Left
                    }
                    _ => LeaveReason::Lost,
                };
            }
        }
    };
    let mut shared = lock(&shared);
    let owner = shared.members.get(&member).is_some_and(|entry| {
        entry
            .writer
            .as_ref()
            .is_some_and(|writer| writer.connection.stable_id() == connection.stable_id())
    });
    if owner && shared.state == State::Open {
        let entry = shared.members.get_mut(&member).expect("checked above");
        entry.connected = false;
        entry.writer = None;
        shared.events.push(Event::Left { member, reason });
        shared.broadcast_roster();
    }
}

async fn run_guest(shared: Arc<Mutex<Shared>>, config: JoinConfig) -> (EndReason, String) {
    let relay = config.invitation.relay_mode();
    let endpoint = match bind(&config.identity, &relay, config.relay_only, false).await {
        Ok(endpoint) => endpoint,
        Err(cause) => return (EndReason::Failed, cause),
    };
    lock(&shared).endpoint = Some(endpoint.clone());
    let connection = match until_relayed(
        &endpoint,
        endpoint.connect(config.invitation.addr.clone(), ALPN),
    )
    .await
    {
        Ok(Ok(connection)) => connection,
        Ok(Err(cause)) => {
            return (
                EndReason::Unreachable,
                format!("could not reach the host: {cause}"),
            )
        }
        Err(reason) => return (EndReason::Unreachable, reason),
    };
    let (send, mut recv) = match connection.open_bi().await {
        Ok(streams) => streams,
        Err(cause) => return (EndReason::HostLost, cause.to_string()),
    };
    let writer = writer(send, connection.clone(), Arc::new(AtomicU32::new(OPEN)));
    writer.send(&Frame::Hello {
        protocol: PROTOCOL,
        application: config.application.clone(),
        secret: config.invitation.secret.clone(),
    });
    match tokio::time::timeout(HELLO_TIMEOUT, read_frame(&mut recv)).await {
        Ok(Ok(Some(Frame::Welcome { member, chat, .. }))) => {
            let mut shared = lock(&shared);
            if shared.state == State::Ended {
                return (EndReason::Left, String::new());
            }
            shared.state = State::Open;
            shared.local_member = member;
            shared.chat = chat
                .into_iter()
                .map(|line| ChatLine {
                    sequence: line.sequence,
                    local_id: line.local_id,
                    member: line.member,
                    state: ChatState::Delivered,
                    text: line.text,
                })
                .collect();
            shared.chat_revision += 1;
            shared.members.insert(
                HOST_MEMBER,
                Member {
                    key: connection.remote_id().to_string(),
                    is_host: true,
                    connected: true,
                    awaiting_view: false,
                    writer: Some(writer),
                },
            );
            shared.events.push(Event::Opened);
        }
        Ok(Ok(Some(Frame::Refused { reason }))) => return (EndReason::Refused, reason),
        Ok(Err(cause)) => return (EndReason::HostLost, cause),
        _ => {
            return (
                EndReason::Unreachable,
                "the host did not welcome this guest".to_owned(),
            )
        }
    }
    loop {
        let frame = match read_frame(&mut recv).await {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                return (
                    EndReason::HostClosed,
                    "the host ended the session".to_owned(),
                )
            }
            Err(cause) => {
                return match connection.close_reason() {
                    // The host's game closed its endpoint.
                    Some(iroh::endpoint::ConnectionError::ApplicationClosed(_)) => (
                        EndReason::HostClosed,
                        "the host ended the session".to_owned(),
                    ),
                    Some(reason) => (EndReason::HostLost, reason.to_string()),
                    None => (EndReason::HostLost, cause),
                };
            }
        };
        let mut shared = lock(&shared);
        let within = match frame {
            Frame::Message {
                sequence, payload, ..
            } => {
                shared.host_sequence = sequence;
                let bytes = payload.len();
                shared.receive(
                    Event::Message {
                        member: HOST_MEMBER,
                        sequence,
                        seen: 0,
                        payload,
                    },
                    bytes,
                )
            }
            Frame::View { sequence, payload } => {
                shared.host_sequence = sequence;
                let bytes = payload.len();
                shared.receive(Event::View { sequence, payload }, bytes)
            }
            Frame::Roster(roster) => {
                let local = shared.local_member;
                let host = shared.members.remove(&HOST_MEMBER);
                shared.members.clear();
                for entry in roster {
                    shared.members.insert(
                        entry.member,
                        Member {
                            key: entry.key,
                            is_host: entry.is_host,
                            connected: entry.connected || entry.member == local,
                            awaiting_view: false,
                            writer: None,
                        },
                    );
                }
                if let Some(host) = host {
                    shared.members.insert(HOST_MEMBER, host);
                }
                true
            }
            Frame::ChatLine(line) => {
                let local = shared.local_member;
                shared.chat.retain(|own| {
                    !(own.state == ChatState::Pending
                        && line.member == local
                        && own.local_id == line.local_id)
                });
                shared.chat.push(ChatLine {
                    sequence: line.sequence,
                    local_id: line.local_id,
                    member: line.member,
                    state: ChatState::Delivered,
                    text: line.text,
                });
                if shared.chat.len() > CHAT_LINES {
                    shared.chat.remove(0);
                }
                shared.chat_revision += 1;
                true
            }
            Frame::Bye => {
                drop(shared);
                return (
                    EndReason::HostClosed,
                    "the host ended the session".to_owned(),
                );
            }
            _ => true,
        };
        if !within {
            connection.close(CLOSE_FLOODED.into(), b"");
            return (
                EndReason::HostLost,
                "the host sent more than this guest could hold".to_owned(),
            );
        }
    }
}

#[cfg(test)]
mod tests;
