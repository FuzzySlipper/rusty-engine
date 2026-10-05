//! Small-party multiplayer: one host and a handful of guests. The Engine owns
//! the connections, membership, delivery and chat; the product owns what its
//! messages and views mean. Network work runs on Engine threads, and its
//! observations are refreshed when each product call begins.
use crate::*;
use std::ffi::c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSessionHandle {
    pub value: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionHostRequest {
    /// Names the local identity whose key members follow across rejoins
    /// (letters, digits, `-`, `_`). Stored under the persistence root.
    pub identity: NativeUtf8Slice,
    /// The product's compatibility tag; guests must present the same one.
    pub application: NativeUtf8Slice,
    /// A relay URL, `n0` for number 0's public development relays, or empty
    /// for direct connections only.
    pub relay: NativeUtf8Slice,
    /// The access token a self-hosted relay requires; empty for none. Guests
    /// receive it in the invitation.
    pub relay_token: NativeUtf8Slice,
    /// Never use a direct path, so peers see only the relay's address.
    pub relay_only: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionJoinRequest {
    pub identity: NativeUtf8Slice,
    pub application: NativeUtf8Slice,
    /// The text `ReadInvitation` gave the host.
    pub invitation: NativeUtf8Slice,
    pub relay_only: bool,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionState {
    Starting = 0,
    Open = 1,
    Ended = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionRole {
    Host = 0,
    Guest = 1,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionEndReason {
    None = 0,
    /// This side left.
    Left = 1,
    /// The host ended the session.
    HostClosed = 2,
    /// The connection to the host broke or timed out.
    HostLost = 3,
    /// The host refused the join: another session, application or protocol.
    Refused = 4,
    /// The host or relay could not be reached.
    Unreachable = 5,
    /// This side could not start.
    Failed = 6,
}

/// The session as of the start of the current product call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionReadout {
    pub state: NativeSessionState,
    pub role: NativeSessionRole,
    pub end_reason: NativeSessionEndReason,
    /// This side's member number; zero until open. The host is member 1.
    pub local_member: u32,
    /// Host: the last sequence it sent. Guest: the last it received.
    pub host_sequence: u64,
    /// Changes whenever the chat transcript does.
    pub chat_revision: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionPath {
    None = 0,
    Direct = 1,
    Relay = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionMember {
    pub member: u32,
    /// The member's public key: stable across rejoins of one identity.
    pub key: NativeUtf8Slice,
    pub is_host: bool,
    pub is_local: bool,
    pub connected: bool,
    /// Host: joined or rejoined, and not yet sent a view.
    pub awaiting_view: bool,
    /// The path this side uses to reach the member, if it holds one.
    pub path: NativeSessionPath,
    pub rtt_micros: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionMemberResult {
    pub members: *const NativeSessionMember,
    pub members_len: usize,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionEventKind {
    Opened = 0,
    MemberJoined = 1,
    MemberRejoined = 2,
    MemberLeft = 3,
    Message = 4,
    View = 5,
    Ended = 6,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionLeaveReason {
    None = 0,
    Left = 1,
    Lost = 2,
    TooSlow = 3,
    Flooded = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionEvent {
    pub kind: NativeSessionEventKind,
    pub member: u32,
    /// A guest's message or view: the host's sequence for it.
    pub sequence: u64,
    /// A message on the host: the last host sequence its sender had received.
    pub seen: u64,
    pub leave_reason: NativeSessionLeaveReason,
    pub end_reason: NativeSessionEndReason,
    pub payload: NativeByteSlice,
}

/// Events since the previous `TakeEvents`, borrowed until the next Session
/// call. Each event is returned once.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionEventResult {
    pub events: *const NativeSessionEvent,
    pub events_len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionSendRequest {
    pub session: NativeSessionHandle,
    /// Host: the recipient. Guest: 1, the host.
    pub member: u32,
    pub payload: NativeByteSlice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionBroadcastRequest {
    pub session: NativeSessionHandle,
    pub payload: NativeByteSlice,
}

/// `recipients` is how many members the frame was queued for; zero means it
/// was not sent and will not be.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSessionSendReceipt {
    pub sequence: u64,
    pub recipients: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionChatRequest {
    pub session: NativeSessionHandle,
    pub text: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeSessionChatReceipt {
    pub local_id: u64,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSessionChatState {
    Pending = 0,
    Delivered = 1,
    Failed = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionChatLine {
    /// The host's transcript sequence; zero while pending or failed.
    pub sequence: u64,
    /// The sender's id for the line, as `SendChat` returned it.
    pub local_id: u64,
    pub member: u32,
    pub state: NativeSessionChatState,
    pub text: NativeUtf8Slice,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionChatResult {
    pub lines: *const NativeSessionChatLine,
    pub lines_len: usize,
}

pub type NativeHostSession = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSessionHostRequest,
    *mut NativeSessionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeJoinSession = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSessionJoinRequest,
    *mut NativeSessionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
/// Leaves: peers are told, and the handle is released.
pub type NativeDestroySession = unsafe extern "C" fn(*mut c_void, NativeSessionHandle) -> i32;
pub type NativeReadSession = unsafe extern "C" fn(
    *mut c_void,
    NativeSessionHandle,
    *mut NativeSessionReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSessionText = unsafe extern "C" fn(
    *mut c_void,
    NativeSessionHandle,
    *mut NativeByteResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSessionMembers = unsafe extern "C" fn(
    *mut c_void,
    NativeSessionHandle,
    *mut NativeSessionMemberResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeTakeSessionEvents = unsafe extern "C" fn(
    *mut c_void,
    NativeSessionHandle,
    *mut NativeSessionEventResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSendSessionMessage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSessionSendRequest,
    *mut NativeSessionSendReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeBroadcastSessionMessage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSessionBroadcastRequest,
    *mut NativeSessionSendReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSendSessionChat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSessionChatRequest,
    *mut NativeSessionChatReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSessionChat = unsafe extern "C" fn(
    *mut c_void,
    NativeSessionHandle,
    *mut NativeSessionChatResult,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSessionApi {
    pub context: *mut c_void,
    pub host: NativeHostSession,
    pub join: NativeJoinSession,
    pub destroy: NativeDestroySession,
    pub read: NativeReadSession,
    /// The invitation text guests join with; empty until the host is open.
    pub read_invitation: NativeReadSessionText,
    /// UTF-8 text explaining why the session ended; empty otherwise.
    pub read_diagnostic: NativeReadSessionText,
    pub read_members: NativeReadSessionMembers,
    pub take_events: NativeTakeSessionEvents,
    /// Host: to one member. Guest: to the host.
    pub send: NativeSendSessionMessage,
    /// Host only: to every connected member that has its view.
    pub broadcast: NativeBroadcastSessionMessage,
    /// Host only: a fresh view for one member; broadcasts reach a joined or
    /// rejoined member only after it.
    pub send_view: NativeSendSessionMessage,
    pub send_chat: NativeSendSessionChat,
    pub read_chat: NativeReadSessionChat,
}
