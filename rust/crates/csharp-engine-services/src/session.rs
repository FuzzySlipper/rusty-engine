//! Small-party multiplayer for the product: `svc-session` behind the generated
//! table. Each product call begins with a fresh snapshot of every session;
//! received events wait in the bridge until the product takes them, so a call
//! that does not look (a debug command, say) loses none.
use std::{collections::BTreeMap, ffi::c_void, path::PathBuf, sync::Arc};

use csharp_engine_abi::*;
use svc_session::{
    ChatState, Drained, EndReason, Event, HostConfig, Identity, Invitation, JoinConfig,
    LeaveReason, PathKind, Relay, Role, Session, SessionRuntime, State,
};

use crate::{
    composition::{borrowed_utf8, ABI_OK},
    operation_diagnostics::{clear_receipt, BorrowedResult, OperationDiagnostics},
    CsharpEngineServicesError,
};

struct Entry {
    session: Session,
    snapshot: Drained,
    events: Vec<Event>,
}

pub(crate) struct RuntimeSessionBridge {
    /// Identities are stored beneath it; without one they last a process.
    persistence_root: Option<PathBuf>,
    /// Started by the first Host or Join, so a product that never plays with
    /// others runs no network work.
    runtime: Option<SessionRuntime>,
    sessions: BTreeMap<u64, Entry>,
    next: u64,
    diagnostics: OperationDiagnostics,
    borrowed: BorrowedResult,
}

fn error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_SESSION", message)
}

impl RuntimeSessionBridge {
    pub(crate) fn new(persistence_root: Option<PathBuf>) -> Self {
        Self {
            persistence_root,
            runtime: None,
            sessions: BTreeMap::new(),
            next: 1,
            diagnostics: OperationDiagnostics::default(),
            borrowed: BorrowedResult::default(),
        }
    }

    pub(crate) fn begin_call(&mut self) {
        for entry in self.sessions.values_mut() {
            let mut snapshot = entry.session.drain();
            entry.events.append(&mut snapshot.events);
            entry.snapshot = snapshot;
        }
    }

    fn identity(&self, name: &str) -> Result<Identity, CsharpEngineServicesError> {
        let storable = !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        if !storable {
            return Err(error(format!(
                "`{name}` is not an identity name: use 1 to 64 letters, digits, `-` or `_`"
            )));
        }
        match &self.persistence_root {
            Some(root) => Identity::stored(
                &root
                    .join("engine-session-identities")
                    .join(format!("{name}.key")),
            )
            .map_err(error),
            None => Ok(Identity::ephemeral()),
        }
    }

    fn insert(&mut self, session: Session) -> NativeSessionHandle {
        let snapshot = session.drain();
        let value = self.next;
        self.next += 1;
        self.sessions.insert(
            value,
            Entry {
                session,
                events: Vec::new(),
                snapshot,
            },
        );
        NativeSessionHandle { value }
    }

    fn runtime(&mut self) -> Result<&SessionRuntime, CsharpEngineServicesError> {
        if self.runtime.is_none() {
            self.runtime = Some(SessionRuntime::new().map_err(error)?);
        }
        Ok(self.runtime.as_ref().expect("started above"))
    }

    fn host(
        &mut self,
        request: &NativeSessionHostRequest,
    ) -> Result<NativeSessionHandle, CsharpEngineServicesError> {
        let identity = self.identity(utf8(request.identity, "identity")?)?;
        let config = HostConfig {
            identity,
            application: utf8(request.application, "application")?.to_owned(),
            relay: Relay::parse(utf8(request.relay, "relay")?).map_err(error)?,
            relay_token: utf8(request.relay_token, "relay token")?.to_owned(),
            relay_only: request.relay_only,
        };
        if config.relay_only && config.relay == Relay::None {
            return Err(error("a relay-only session needs a relay"));
        }
        let session = Session::host(self.runtime()?, config);
        Ok(self.insert(session))
    }

    fn join(
        &mut self,
        request: &NativeSessionJoinRequest,
    ) -> Result<NativeSessionHandle, CsharpEngineServicesError> {
        let identity = self.identity(utf8(request.identity, "identity")?)?;
        let invitation =
            Invitation::decode(utf8(request.invitation, "invitation")?).map_err(error)?;
        let config = JoinConfig {
            identity,
            application: utf8(request.application, "application")?.to_owned(),
            invitation,
            relay_only: request.relay_only,
        };
        let session = Session::join(self.runtime()?, config);
        Ok(self.insert(session))
    }

    fn entry(
        &mut self,
        handle: NativeSessionHandle,
    ) -> Result<&mut Entry, CsharpEngineServicesError> {
        self.sessions
            .get_mut(&handle.value)
            .ok_or_else(|| error("unknown session"))
    }

    fn read(
        &mut self,
        handle: NativeSessionHandle,
    ) -> Result<NativeSessionReadout, CsharpEngineServicesError> {
        let snapshot = &self.entry(handle)?.snapshot;
        Ok(NativeSessionReadout {
            state: match snapshot.state {
                State::Starting => NativeSessionState::Starting,
                State::Open => NativeSessionState::Open,
                State::Ended => NativeSessionState::Ended,
            },
            role: match snapshot.role {
                Role::Host => NativeSessionRole::Host,
                Role::Guest => NativeSessionRole::Guest,
            },
            end_reason: end_reason(snapshot.end_reason),
            local_member: snapshot.local_member,
            host_sequence: snapshot.host_sequence,
            chat_revision: snapshot.chat_revision,
        })
    }

    fn read_text(
        &mut self,
        handle: NativeSessionHandle,
        diagnostic: bool,
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
        let snapshot = &self.entry(handle)?.snapshot;
        let text: Arc<str> = Arc::from(if diagnostic {
            snapshot.diagnostic.as_str()
        } else {
            snapshot.invitation.as_str()
        });
        let result = NativeByteResult {
            bytes: text.as_ptr(),
            len: text.len(),
        };
        self.borrowed.hold(text);
        Ok(result)
    }

    fn read_members(
        &mut self,
        handle: NativeSessionHandle,
    ) -> Result<NativeSessionMemberResult, CsharpEngineServicesError> {
        let members = self.entry(handle)?.snapshot.members.clone();
        let values: Vec<NativeSessionMember> = members
            .iter()
            .map(|member| NativeSessionMember {
                member: member.member,
                key: slice(member.key.as_bytes()),
                is_host: member.is_host,
                is_local: member.is_local,
                connected: member.connected,
                awaiting_view: member.awaiting_view,
                path: match member.path {
                    PathKind::None => NativeSessionPath::None,
                    PathKind::Direct => NativeSessionPath::Direct,
                    PathKind::Relay => NativeSessionPath::Relay,
                },
                rtt_micros: u32::try_from(member.rtt.as_micros()).unwrap_or(u32::MAX),
            })
            .collect();
        let result = NativeSessionMemberResult {
            members: values.as_ptr(),
            members_len: values.len(),
        };
        self.borrowed.hold((members, values));
        Ok(result)
    }

    fn take_events(
        &mut self,
        handle: NativeSessionHandle,
    ) -> Result<NativeSessionEventResult, CsharpEngineServicesError> {
        let entry = self.entry(handle)?;
        let events = std::mem::take(&mut entry.events);
        // Only now has the product the payloads; until then they stay charged
        // against the session's untaken limit.
        entry.session.release(events.iter().map(payload_len).sum());
        let values: Vec<NativeSessionEvent> = events.iter().map(native_event).collect();
        let result = NativeSessionEventResult {
            events: values.as_ptr(),
            events_len: values.len(),
        };
        self.borrowed.hold((events, values));
        Ok(result)
    }

    fn send(
        &mut self,
        request: &NativeSessionSendRequest,
        view: bool,
    ) -> Result<NativeSessionSendReceipt, CsharpEngineServicesError> {
        let payload = bytes(request.payload)?;
        let session = &self.entry(request.session)?.session;
        let receipt = if view {
            session.send_view(request.member, payload)
        } else {
            session.send(request.member, payload)
        }
        .map_err(error)?;
        Ok(NativeSessionSendReceipt {
            sequence: receipt.sequence,
            recipients: receipt.recipients,
        })
    }

    fn broadcast(
        &mut self,
        request: &NativeSessionBroadcastRequest,
    ) -> Result<NativeSessionSendReceipt, CsharpEngineServicesError> {
        let payload = bytes(request.payload)?;
        let receipt = self
            .entry(request.session)?
            .session
            .broadcast(payload)
            .map_err(error)?;
        Ok(NativeSessionSendReceipt {
            sequence: receipt.sequence,
            recipients: receipt.recipients,
        })
    }

    fn send_chat(
        &mut self,
        request: &NativeSessionChatRequest,
    ) -> Result<NativeSessionChatReceipt, CsharpEngineServicesError> {
        let text = utf8(request.text, "chat text")?;
        let entry = self.entry(request.session)?;
        let local_id = entry.session.send_chat(text).map_err(error)?;
        // The line belongs in this call's transcript, sent or failed.
        let mut snapshot = entry.session.drain();
        entry.events.append(&mut snapshot.events);
        entry.snapshot = snapshot;
        Ok(NativeSessionChatReceipt { local_id })
    }

    fn read_chat(
        &mut self,
        handle: NativeSessionHandle,
    ) -> Result<NativeSessionChatResult, CsharpEngineServicesError> {
        let lines = self.entry(handle)?.snapshot.chat.clone();
        let values: Vec<NativeSessionChatLine> = lines
            .iter()
            .map(|line| NativeSessionChatLine {
                sequence: line.sequence,
                local_id: line.local_id,
                member: line.member,
                state: match line.state {
                    ChatState::Pending => NativeSessionChatState::Pending,
                    ChatState::Delivered => NativeSessionChatState::Delivered,
                    ChatState::Failed => NativeSessionChatState::Failed,
                },
                text: slice(line.text.as_bytes()),
            })
            .collect();
        let result = NativeSessionChatResult {
            lines: values.as_ptr(),
            lines_len: values.len(),
        };
        self.borrowed.hold((lines, values));
        Ok(result)
    }
}

fn end_reason(reason: Option<EndReason>) -> NativeSessionEndReason {
    match reason {
        None => NativeSessionEndReason::None,
        Some(EndReason::Left) => NativeSessionEndReason::Left,
        Some(EndReason::HostClosed) => NativeSessionEndReason::HostClosed,
        Some(EndReason::HostLost) => NativeSessionEndReason::HostLost,
        Some(EndReason::Refused) => NativeSessionEndReason::Refused,
        Some(EndReason::Unreachable) => NativeSessionEndReason::Unreachable,
        Some(EndReason::Failed) => NativeSessionEndReason::Failed,
    }
}

fn payload_len(event: &Event) -> usize {
    match event {
        Event::Message { payload, .. } | Event::View { payload, .. } => payload.len(),
        _ => 0,
    }
}

fn native_event(event: &Event) -> NativeSessionEvent {
    let mut native = NativeSessionEvent {
        kind: NativeSessionEventKind::Opened,
        member: 0,
        sequence: 0,
        seen: 0,
        leave_reason: NativeSessionLeaveReason::None,
        end_reason: NativeSessionEndReason::None,
        payload: NativeByteSlice {
            bytes: std::ptr::null(),
            len: 0,
        },
    };
    match event {
        Event::Opened => {}
        Event::Joined { member } => {
            native.kind = NativeSessionEventKind::MemberJoined;
            native.member = *member;
        }
        Event::Rejoined { member } => {
            native.kind = NativeSessionEventKind::MemberRejoined;
            native.member = *member;
        }
        Event::Left { member, reason } => {
            native.kind = NativeSessionEventKind::MemberLeft;
            native.member = *member;
            native.leave_reason = match reason {
                LeaveReason::Left => NativeSessionLeaveReason::Left,
                LeaveReason::Lost => NativeSessionLeaveReason::Lost,
                LeaveReason::TooSlow => NativeSessionLeaveReason::TooSlow,
                LeaveReason::Flooded => NativeSessionLeaveReason::Flooded,
            };
        }
        Event::Message {
            member,
            sequence,
            seen,
            payload,
        } => {
            native.kind = NativeSessionEventKind::Message;
            native.member = *member;
            native.sequence = *sequence;
            native.seen = *seen;
            native.payload = NativeByteSlice {
                bytes: payload.as_ptr(),
                len: payload.len(),
            };
        }
        Event::View { sequence, payload } => {
            native.kind = NativeSessionEventKind::View;
            native.member = svc_session::HOST_MEMBER;
            native.sequence = *sequence;
            native.payload = NativeByteSlice {
                bytes: payload.as_ptr(),
                len: payload.len(),
            };
        }
        Event::Ended { reason } => {
            native.kind = NativeSessionEventKind::Ended;
            native.end_reason = end_reason(Some(*reason));
        }
    }
    native
}

/// The borrowed text, valid only during the ABI call that supplied it.
fn utf8<'call>(
    value: NativeUtf8Slice,
    field: &'static str,
) -> Result<&'call str, CsharpEngineServicesError> {
    unsafe { borrowed_utf8(value.bytes, value.len, field) }
}

/// The borrowed bytes, valid only during the ABI call that supplied them.
fn bytes<'call>(value: NativeByteSlice) -> Result<&'call [u8], CsharpEngineServicesError> {
    if value.len == 0 {
        return Ok(&[]);
    }
    if value.bytes.is_null() {
        return Err(error("the payload had a length without bytes"));
    }
    Ok(unsafe { std::slice::from_raw_parts(value.bytes, value.len) })
}

fn slice(bytes: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: bytes.as_ptr(),
        len: bytes.len(),
    }
}

/// Runs one refusable operation: writes its result or reports its refusal
/// through the receipt.
unsafe fn refusable<Input, Output>(
    context: *mut c_void,
    input: Input,
    result: *mut Output,
    receipt: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(
        &mut RuntimeSessionBridge,
        Input,
    ) -> Result<Output, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(receipt);
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeSessionBridge>() };
    match operation(bridge, input) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.diagnostics.retain(&error, receipt);
            0
        }
    }
}

/// Dereferences a borrowed request, or refuses a null one.
unsafe fn request<'a, T>(request: *const T) -> Result<&'a T, CsharpEngineServicesError> {
    unsafe { request.as_ref() }.ok_or_else(|| error("the request was null"))
}

macro_rules! request_operation {
    ($name:ident, $request:ty, $output:ty, |$bridge:ident, $input:ident| $body:expr) => {
        unsafe extern "C" fn $name(
            context: *mut c_void,
            input: *const $request,
            result: *mut $output,
            receipt: *mut NativeOperationErrorReceipt,
        ) -> i32 {
            unsafe {
                refusable(context, input, result, receipt, |$bridge, input| {
                    let $input = request(input)?;
                    $body
                })
            }
        }
    };
}

macro_rules! handle_operation {
    ($name:ident, $output:ty, |$bridge:ident, $handle:ident| $body:expr) => {
        unsafe extern "C" fn $name(
            context: *mut c_void,
            handle: NativeSessionHandle,
            result: *mut $output,
            receipt: *mut NativeOperationErrorReceipt,
        ) -> i32 {
            unsafe { refusable(context, handle, result, receipt, |$bridge, $handle| $body) }
        }
    };
}

request_operation!(
    host,
    NativeSessionHostRequest,
    NativeSessionHandle,
    |bridge, input| bridge.host(input)
);
request_operation!(
    join,
    NativeSessionJoinRequest,
    NativeSessionHandle,
    |bridge, input| bridge.join(input)
);
request_operation!(
    send,
    NativeSessionSendRequest,
    NativeSessionSendReceipt,
    |bridge, input| bridge.send(input, false)
);
request_operation!(
    send_view,
    NativeSessionSendRequest,
    NativeSessionSendReceipt,
    |bridge, input| bridge.send(input, true)
);
request_operation!(
    broadcast,
    NativeSessionBroadcastRequest,
    NativeSessionSendReceipt,
    |bridge, input| bridge.broadcast(input)
);
request_operation!(
    send_chat,
    NativeSessionChatRequest,
    NativeSessionChatReceipt,
    |bridge, input| bridge.send_chat(input)
);
handle_operation!(read, NativeSessionReadout, |bridge, handle| bridge
    .read(handle));
handle_operation!(read_invitation, NativeByteResult, |bridge, handle| bridge
    .read_text(handle, false));
handle_operation!(read_diagnostic, NativeByteResult, |bridge, handle| bridge
    .read_text(handle, true));
handle_operation!(read_members, NativeSessionMemberResult, |bridge, handle| {
    bridge.read_members(handle)
});
handle_operation!(take_events, NativeSessionEventResult, |bridge, handle| {
    bridge.take_events(handle)
});
handle_operation!(read_chat, NativeSessionChatResult, |bridge, handle| bridge
    .read_chat(handle));

/// Leaves the session: peers are told, and the handle is released.
unsafe extern "C" fn destroy(context: *mut c_void, handle: NativeSessionHandle) -> i32 {
    match unsafe { context.cast::<RuntimeSessionBridge>().as_mut() } {
        Some(bridge) => {
            if bridge.sessions.remove(&handle.value).is_some() {
                ABI_OK
            } else {
                0
            }
        }
        None => 0,
    }
}

pub(crate) fn api(bridge: &mut RuntimeSessionBridge) -> NativeSessionApi {
    NativeSessionApi {
        context: (bridge as *mut RuntimeSessionBridge).cast(),
        host,
        join,
        destroy,
        read,
        read_invitation,
        read_diagnostic,
        read_members,
        take_events,
        send,
        broadcast,
        send_view,
        send_chat,
        read_chat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_diagnostics::{empty_receipt, receipt_codes};
    use std::time::{Duration, Instant};

    fn text(value: &str) -> NativeUtf8Slice {
        slice(value.as_bytes())
    }

    fn settle(
        bridge: &mut RuntimeSessionBridge,
        handle: NativeSessionHandle,
        done: impl Fn(&NativeSessionReadout) -> bool,
    ) -> NativeSessionReadout {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            bridge.begin_call();
            let readout = bridge.read(handle).unwrap();
            if done(&readout) || Instant::now() > deadline {
                return readout;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn invitation(bridge: &mut RuntimeSessionBridge, handle: NativeSessionHandle) -> String {
        let result = bridge.read_text(handle, false).unwrap();
        String::from_utf8(unsafe { std::slice::from_raw_parts(result.bytes, result.len) }.to_vec())
            .unwrap()
    }

    #[test]
    fn a_product_hosts_joins_and_takes_each_event_once() {
        let root = tempfile::tempdir().unwrap();
        let mut host_bridge = RuntimeSessionBridge::new(Some(root.path().to_owned()));
        let mut guest_bridge = RuntimeSessionBridge::new(Some(root.path().to_owned()));
        assert!(
            host_bridge.runtime.is_none(),
            "nothing runs before a session"
        );
        let host = host_bridge
            .host(&NativeSessionHostRequest {
                identity: text("host"),
                application: text("fixture/1"),
                relay: text(""),
                relay_token: text(""),
                relay_only: false,
            })
            .unwrap();
        let open = settle(&mut host_bridge, host, |readout| {
            readout.state != NativeSessionState::Starting
        });
        assert_eq!(open.state, NativeSessionState::Open);
        assert_eq!(open.local_member, 1);
        let invitation = invitation(&mut host_bridge, host);
        let guest = guest_bridge
            .join(&NativeSessionJoinRequest {
                identity: text("guest"),
                application: text("fixture/1"),
                invitation: text(&invitation),
                relay_only: false,
            })
            .unwrap();
        settle(&mut guest_bridge, guest, |readout| {
            readout.state == NativeSessionState::Open
        });
        let mut joined = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while joined.is_empty() && Instant::now() < deadline {
            host_bridge.begin_call();
            let events = host_bridge.take_events(host).unwrap();
            joined = unsafe { std::slice::from_raw_parts(events.events, events.events_len) }
                .iter()
                .filter(|event| event.kind == NativeSessionEventKind::MemberJoined)
                .map(|event| event.member)
                .collect();
        }
        assert_eq!(joined, [2]);
        host_bridge.begin_call();
        assert_eq!(
            host_bridge.take_events(host).unwrap().events_len,
            0,
            "events are taken once"
        );

        let payload = b"hello";
        let receipt = guest_bridge
            .send(
                &NativeSessionSendRequest {
                    session: guest,
                    member: 1,
                    payload: NativeByteSlice {
                        bytes: payload.as_ptr(),
                        len: payload.len(),
                    },
                },
                false,
            )
            .unwrap();
        assert_eq!(receipt.recipients, 1);
        let mut received = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while received.is_empty() && Instant::now() < deadline {
            host_bridge.begin_call();
            let events = host_bridge.take_events(host).unwrap();
            for event in unsafe { std::slice::from_raw_parts(events.events, events.events_len) } {
                if event.kind == NativeSessionEventKind::Message {
                    received = unsafe {
                        std::slice::from_raw_parts(event.payload.bytes, event.payload.len)
                    }
                    .to_vec();
                }
            }
        }
        assert_eq!(received, b"hello");

        let members = host_bridge.read_members(host).unwrap();
        let members = unsafe { std::slice::from_raw_parts(members.members, members.members_len) };
        assert_eq!(members.len(), 2);
        assert!(members[1].awaiting_view && members[1].path == NativeSessionPath::Direct);

        // Identities persist under the root, so a rejoin keeps its member.
        assert!(root
            .path()
            .join("engine-session-identities/guest.key")
            .exists());

        let api = api(&mut guest_bridge);
        assert_eq!(unsafe { (api.destroy)(api.context, guest) }, ABI_OK);
        let mut readout = NativeSessionReadout {
            state: NativeSessionState::Starting,
            role: NativeSessionRole::Host,
            end_reason: NativeSessionEndReason::None,
            local_member: 0,
            host_sequence: 0,
            chat_revision: 0,
        };
        let mut receipt = empty_receipt();
        assert_eq!(
            unsafe { (api.read)(api.context, guest, &mut readout, &mut receipt) },
            0
        );
        assert_eq!(receipt_codes(&receipt), ["CSHARP_SESSION"]);
    }

    #[test]
    fn payload_stays_charged_across_calls_until_taken() {
        let mut host_bridge = RuntimeSessionBridge::new(None);
        let mut guest_bridge = RuntimeSessionBridge::new(None);
        let host = host_bridge
            .host(&NativeSessionHostRequest {
                identity: text("host"),
                application: text("fixture/1"),
                relay: text(""),
                relay_token: text(""),
                relay_only: false,
            })
            .unwrap();
        settle(&mut host_bridge, host, |readout| {
            readout.state == NativeSessionState::Open
        });
        let invitation = invitation(&mut host_bridge, host);
        let guest = guest_bridge
            .join(&NativeSessionJoinRequest {
                identity: text("guest"),
                application: text("fixture/1"),
                invitation: text(&invitation),
                relay_only: false,
            })
            .unwrap();
        settle(&mut guest_bridge, guest, |readout| {
            readout.state == NativeSessionState::Open
        });
        let payload = [3u8; 1000];
        for _ in 0..3 {
            guest_bridge
                .send(
                    &NativeSessionSendRequest {
                        session: guest,
                        member: 1,
                        payload: NativeByteSlice {
                            bytes: payload.as_ptr(),
                            len: payload.len(),
                        },
                    },
                    false,
                )
                .unwrap();
        }
        // Several calls begin, none takes its events.
        let deadline = Instant::now() + Duration::from_secs(10);
        let untaken = |bridge: &mut RuntimeSessionBridge| {
            bridge.sessions[&host.value].session.untaken_bytes()
        };
        while untaken(&mut host_bridge) < 3000 && Instant::now() < deadline {
            host_bridge.begin_call();
            std::thread::sleep(Duration::from_millis(5));
        }
        host_bridge.begin_call();
        assert_eq!(
            untaken(&mut host_bridge),
            3000,
            "drained payload stays charged"
        );
        host_bridge.take_events(host).unwrap();
        assert_eq!(untaken(&mut host_bridge), 0, "taking releases it");
    }

    #[test]
    fn requests_name_a_storable_identity_and_a_real_invitation() {
        let mut bridge = RuntimeSessionBridge::new(None);
        for identity in ["", "../escape", "has space"] {
            assert!(bridge.identity(identity).is_err(), "{identity}");
        }
        assert!(bridge
            .join(&NativeSessionJoinRequest {
                identity: text("guest"),
                application: text("fixture/1"),
                invitation: text("not an invitation"),
                relay_only: false,
            })
            .is_err());
        assert!(bridge
            .host(&NativeSessionHostRequest {
                identity: text("host"),
                application: text("fixture/1"),
                relay: text(""),
                relay_token: text(""),
                relay_only: true,
            })
            .is_err());
    }
}
