//! The session's frames: a little-endian u32 length, then a kind byte and its
//! fields. Strings and payloads are u32-length prefixed.

use iroh::endpoint::{RecvStream, SendStream};

/// Raised when frames change incompatibly; a mismatched guest is refused.
pub(crate) const PROTOCOL: u16 = 1;
/// The largest frame a peer may send. A length beyond it is a broken or
/// hostile peer, not something to allocate for.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Frame {
    /// Guest → host, first frame.
    Hello {
        protocol: u16,
        application: String,
        secret: String,
    },
    /// Host → guest: accepted as `member`, with the roster and transcript.
    Welcome {
        member: u32,
        roster: Vec<RosterEntry>,
        chat: Vec<ChatEntry>,
    },
    Refused {
        reason: String,
    },
    /// Guest → host: `seen` is the last host sequence the guest received.
    Message {
        sequence: u64,
        seen: u64,
        payload: Vec<u8>,
    },
    /// Host → guest: a fresh view; nothing sent before it is needed.
    View {
        sequence: u64,
        payload: Vec<u8>,
    },
    Roster(Vec<RosterEntry>),
    /// Guest → host.
    Chat {
        local_id: u64,
        text: String,
    },
    /// Host → guests: one line of the session transcript.
    ChatLine(ChatEntry),
    /// A graceful departure.
    Bye,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RosterEntry {
    pub member: u32,
    pub key: String,
    pub is_host: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChatEntry {
    pub sequence: u64,
    pub member: u32,
    /// The sender's own id for the line, so the sender can match it.
    pub local_id: u64,
    pub text: String,
}

const HELLO: u8 = 1;
const WELCOME: u8 = 2;
const REFUSED: u8 = 3;
const MESSAGE: u8 = 4;
const VIEW: u8 = 5;
const ROSTER: u8 = 6;
const CHAT: u8 = 7;
const CHAT_LINE: u8 = 8;
const BYE: u8 = 9;

impl Frame {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Writer(vec![0; 4]);
        match self {
            Frame::Hello {
                protocol,
                application,
                secret,
            } => {
                out.u8(HELLO);
                out.u16(*protocol);
                out.text(application);
                out.text(secret);
            }
            Frame::Welcome {
                member,
                roster,
                chat,
            } => {
                out.u8(WELCOME);
                out.u32(*member);
                out.roster(roster);
                out.u32(chat.len() as u32);
                for line in chat {
                    out.chat(line);
                }
            }
            Frame::Refused { reason } => {
                out.u8(REFUSED);
                out.text(reason);
            }
            Frame::Message {
                sequence,
                seen,
                payload,
            } => {
                out.u8(MESSAGE);
                out.u64(*sequence);
                out.u64(*seen);
                out.bytes(payload);
            }
            Frame::View { sequence, payload } => {
                out.u8(VIEW);
                out.u64(*sequence);
                out.bytes(payload);
            }
            Frame::Roster(roster) => {
                out.u8(ROSTER);
                out.roster(roster);
            }
            Frame::Chat { local_id, text } => {
                out.u8(CHAT);
                out.u64(*local_id);
                out.text(text);
            }
            Frame::ChatLine(line) => {
                out.u8(CHAT_LINE);
                out.chat(line);
            }
            Frame::Bye => out.u8(BYE),
        }
        let len = (out.0.len() - 4) as u32;
        out.0[..4].copy_from_slice(&len.to_le_bytes());
        out.0
    }

    pub(crate) fn decode(body: &[u8]) -> Option<Self> {
        let mut input = Reader(body);
        let frame = match input.u8()? {
            HELLO => Frame::Hello {
                protocol: input.u16()?,
                application: input.text()?,
                secret: input.text()?,
            },
            WELCOME => {
                let member = input.u32()?;
                let roster = input.roster()?;
                let count = input.u32()? as usize;
                let mut chat = Vec::with_capacity(count.min(1024));
                for _ in 0..count {
                    chat.push(input.chat()?);
                }
                Frame::Welcome {
                    member,
                    roster,
                    chat,
                }
            }
            REFUSED => Frame::Refused {
                reason: input.text()?,
            },
            MESSAGE => Frame::Message {
                sequence: input.u64()?,
                seen: input.u64()?,
                payload: input.bytes()?.to_vec(),
            },
            VIEW => Frame::View {
                sequence: input.u64()?,
                payload: input.bytes()?.to_vec(),
            },
            ROSTER => Frame::Roster(input.roster()?),
            CHAT => Frame::Chat {
                local_id: input.u64()?,
                text: input.text()?,
            },
            CHAT_LINE => Frame::ChatLine(input.chat()?),
            BYE => Frame::Bye,
            _ => return None,
        };
        input.0.is_empty().then_some(frame)
    }
}

pub(crate) async fn write_frame(send: &mut SendStream, encoded: &[u8]) -> Result<(), String> {
    send.write_all(encoded)
        .await
        .map_err(|cause| cause.to_string())
}

/// Reads one frame. `Ok(None)` is a cleanly finished stream.
pub(crate) async fn read_frame(recv: &mut RecvStream) -> Result<Option<Frame>, String> {
    let mut len = [0u8; 4];
    match recv.read_exact(&mut len).await {
        Ok(()) => {}
        Err(iroh::endpoint::ReadExactError::FinishedEarly(0)) => return Ok(None),
        Err(cause) => return Err(cause.to_string()),
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME + 64 {
        return Err(format!("the peer sent a {len}-byte frame"));
    }
    let mut body = vec![0; len];
    recv.read_exact(&mut body)
        .await
        .map_err(|cause| cause.to_string())?;
    Frame::decode(&body)
        .map(Some)
        .ok_or_else(|| "the peer sent a malformed frame".to_owned())
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, value: u8) {
        self.0.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.0.extend_from_slice(value);
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn roster(&mut self, roster: &[RosterEntry]) {
        self.u32(roster.len() as u32);
        for entry in roster {
            self.u32(entry.member);
            self.text(&entry.key);
            self.u8(u8::from(entry.is_host));
            self.u8(u8::from(entry.connected));
        }
    }
    fn chat(&mut self, line: &ChatEntry) {
        self.u64(line.sequence);
        self.u32(line.member);
        self.u64(line.local_id);
        self.text(&line.text);
    }
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        if self.0.len() < len {
            return None;
        }
        let (head, tail) = self.0.split_at(len);
        self.0 = tail;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn bytes(&mut self) -> Option<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }
    fn text(&mut self) -> Option<String> {
        String::from_utf8(self.bytes()?.to_vec()).ok()
    }
    fn roster(&mut self) -> Option<Vec<RosterEntry>> {
        let count = self.u32()? as usize;
        let mut roster = Vec::with_capacity(count.min(256));
        for _ in 0..count {
            roster.push(RosterEntry {
                member: self.u32()?,
                key: self.text()?,
                is_host: self.u8()? != 0,
                connected: self.u8()? != 0,
            });
        }
        Some(roster)
    }
    fn chat(&mut self) -> Option<ChatEntry> {
        Some(ChatEntry {
            sequence: self.u64()?,
            member: self.u32()?,
            local_id: self.u64()?,
            text: self.text()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_reject_trailing_or_short_bodies() {
        let frames = [
            Frame::Hello {
                protocol: PROTOCOL,
                application: "goldbox/1".into(),
                secret: "s".into(),
            },
            Frame::Welcome {
                member: 3,
                roster: vec![RosterEntry {
                    member: 1,
                    key: "k".into(),
                    is_host: true,
                    connected: true,
                }],
                chat: vec![ChatEntry {
                    sequence: 1,
                    member: 1,
                    local_id: 9,
                    text: "hi".into(),
                }],
            },
            Frame::Message {
                sequence: 4,
                seen: 7,
                payload: vec![1, 2, 3],
            },
            Frame::View {
                sequence: 8,
                payload: vec![],
            },
            Frame::Bye,
        ];
        for frame in frames {
            let encoded = frame.encode();
            assert_eq!(
                u32::from_le_bytes(encoded[..4].try_into().unwrap()) as usize,
                encoded.len() - 4
            );
            assert_eq!(Frame::decode(&encoded[4..]), Some(frame.clone()));
            let mut trailing = encoded[4..].to_vec();
            trailing.push(0);
            assert_eq!(Frame::decode(&trailing), None);
            if encoded.len() > 5 {
                assert_eq!(Frame::decode(&encoded[4..encoded.len() - 1]), None);
            }
        }
    }
}
