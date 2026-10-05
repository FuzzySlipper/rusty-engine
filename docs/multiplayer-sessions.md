# Multiplayer sessions

The `Session` service lets a small group play one game together: one player's
game hosts, a handful of friends join with an invitation, and everyone
exchanges the product's own messages and chats. It is built for relaxed,
host-authoritative games where waiting for a decision is normal, not for
action games.

Multiplayer is opt-in. Nothing starts, binds a socket or contacts a service
until the product calls `Host` or `Join`, so a single-player product needs no
connection, account or configuration.

**The Engine** owns the connections, membership and member identities,
invitations, delivery, the session's chat transcript, and its diagnostics.
**The product** owns everything the messages mean: game state, its
serialization, character assignment, the party leader, votes, turns and save
policy. The session host is whoever called `Host`; a product's party leader is
product state and can be any member.

Mechanism: [`svc-session`](../rust/crates/svc-session) over
[Iroh](https://github.com/n0-computer/iroh) 1.3 (QUIC dialled by public key).
Peers connect directly when they can and through a relay when they cannot, so
no one forwards ports. The bridge is
[`csharp-engine-services/src/session.rs`](../rust/crates/csharp-engine-services/src/session.rs).

## Hosting and joining

```csharp
Session party = engine.Session.Host(new SessionHostRequest(
    Identity: "Hana", Application: "my-game/1", Relay: "n0", RelayToken: "", RelayOnly: false));
// Once Read(party).State is Open:
string invitation = engine.Session.ReadInvitationText(party);

Session guest = engine.Session.Join(new SessionJoinRequest(
    Identity: "Alice", Application: "my-game/1", Invitation: pasted, RelayOnly: false));
```

- **Identity** names a local key, stored under the host-selected persistence
  root (`engine-session-identities/<name>.key`; letters, digits, `-`, `_`).
  A member's key, and so its member number, stays the same whenever it
  rejoins with the same identity. Without a persistence root the key lasts the
  process. Several players on one machine use different names.
- **Application** is the product's compatibility tag. A guest whose tag, or
  Engine session protocol, differs from the host's is refused with a reason.
- **Invitation** is copyable text (`rusty-session-1.…`, a few hundred
  characters) holding the host's public key, its relay, the relay token if
  any, and a session secret. Share it in any chat; no deep link or account is
  involved. It admits anyone holding it until the host's session ends, after
  which a join fails as `Unreachable`. Text that is not an invitation is
  refused at `Join`.
- **Relay** chooses how peers reach the host when no direct path exists:
  - empty: direct connections only, for a LAN; the invitation then carries the
    host's addresses;
  - `n0`: number 0's free public relays. They are rate-limited development
    infrastructure with no service promise, not something to ship on;
  - a URL: a relay you run ([below](#running-a-relay)), with its
    **RelayToken** if it requires one.

  With a relay the invitation carries no address of the host's network; Iroh
  finds a direct path through the relay when one exists and keeps relaying
  when not.
- **RelayOnly** never uses a direct path, so peers see only the relay's
  address. Without it, peers that connect directly learn each other's
  addresses.

The member list says which path each connection uses (`Direct` or `Relay`)
and its round-trip time.

## Observing

Network work runs on Engine threads and never calls the product. Each product
call begins with a fresh snapshot of every session, and all reads in that call
agree:

- `Read`: `State` (`Starting`, `Open`, `Ended`), `Role`, `EndReason`, this
  side's `LocalMember` (the host is member 1), `HostSequence` and
  `ChatRevision`.
- `ReadMembers`: member number, key, host and local flags, `Connected`,
  `AwaitingView`, `Path`, `RttMicros`. Members who left stay listed as not
  connected.
- `TakeEvents`: what happened since the last `TakeEvents`, each event exactly
  once, even across calls that did not look: `Opened`, `MemberJoined`,
  `MemberRejoined`, `MemberLeft` (with a `LeaveReason`), `Message`, `View` and
  `Ended` (with an `EndReason`).
- `ReadDiagnosticText`: why the session ended, in words.

## Delivery

- **Host to members:** `Send` to one member, `Broadcast` to every connected
  member that has its view, and `SendView` (below). **Guest to host:**
  `Send` with member 1 (`SendToHost`). Guests do not message each other; the
  host relays what it chooses.
- Messages to one member arrive reliably and in the order they were sent,
  for as long as that member's connection lasts. The sender of a received
  message is the connection it arrived on, never a claim in the payload.
- A receipt's `Recipients` is how many members the message was queued for.
  Zero means it was not sent and never will be: nothing is held for a
  disconnected member, and nothing is replayed after a reconnect. Whether an
  action happened is the product's state, not a delivery guarantee.
- Every host send takes the next `HostSequence`. A guest's message reaches
  the host with `Seen`, the last host sequence that guest had received, so a
  host can recognise an action made against a view that has since changed.

## Join and rejoin

When a member joins or rejoins, the host sees `MemberJoined` or
`MemberRejoined` and the member is `AwaitingView`. Broadcasts skip it until the
host product sends it a fresh view with `SendView`; the guest receives that as
a `View` event, and everything sent to it afterwards follows in order. A
joining member therefore never needs anything sent before its view. Products
send their whole current state as the view and keep broadcasting complete
compact states as they change; the Engine adds no entity replication or delta
scheme.

A rejoin is a new `Join` with the same identity. The member keeps its member
number and key; the product decides what it reclaims (a character, a seat). A
rejoin that arrives before the old connection timed out replaces it.

## Leaving and loss

Disposing a `Session` leaves: peers learn at once. The host sees
`MemberLeft` with `Left`. Its guests end with `HostClosed`; there is no host
migration.

A peer that vanishes (its game killed, its network gone) is noticed after
about 10 to 12 seconds without traffic: `MemberLeft` with `Lost` on the host,
and `Ended` with `HostLost` on a guest. Other end reasons:
- `Refused`: wrong session, application or protocol;
- `Unreachable`: no relay or host answered, or a relay refused the token;
- `Failed`: this side could not start.

## Chat

`SendChat(session, text)` adds a line and `ReadChat` returns the transcript.
Each line carries the sender's member number, taken from its connection.

- The host keeps the transcript, in the order lines reached it, and sends it
  to each member when it joins or rejoins.
- A guest's own line is `Pending` until it comes back in the host's
  transcript, then `Delivered`. Without a connection to the host it is
  `Failed` and never delivered.
- The transcript lasts as long as the session. Nothing is stored after it
  ends, and a rejoined member gets the host's copy, so nothing is duplicated.

Chat works whenever the session is open, whatever the product is waiting for.
Display names, placement and any chat rules belong to the product. Show chat
as text (`textContent` in the DOM), never as HTML.

## Limits

Each limit answers a failure that internet peers can cause.

| Limit | Value | Why | Cost |
| --- | --- | --- | --- |
| Message payload | 16 MiB | A peer's frame length is untrusted. | Larger states must be split. |
| Queued for one member | 32 MiB | A stalled member would grow the sender's memory without bound. | That member is disconnected, `LeaveReason.TooSlow`. |
| Received, not yet taken | 64 MiB per session | A peer could flood a paused product. | The sender is disconnected (`Flooded`); a flooding host ends the guest's session. |
| Chat line | 4 KiB | Bounds the transcript. | Longer lines are refused. |
| Transcript | 1000 lines | Joiners receive it whole. | Older lines drop off. |
| Silence | 10 s idle | Notice vanished peers. | A peer gone for 10 s is lost, even if it returns. |
| Connect | 30 s | An unreachable host or relay. | |

## Running a relay

A relay passes encrypted traffic between peers that cannot reach each other
directly. It sees which keys talk to which and how much, never the content.
It keeps no game state, so restarting it only drops relayed connections. If
it is down, peers that need it cannot join, and relayed peers are lost after
the idle timeout; directly connected peers carry on.

[`deploy/session-relay`](../deploy/session-relay) runs iroh-relay 1.3, the
open-source relay from the same project:

1. Install the binary:
   `cargo install iroh-relay --version 1.3.0 --features server --locked --root /usr/local`.
2. Point a DNS name at the server. Open TCP 80 and 443, and UDP 7842 for
   address discovery.
3. Copy [`relay.toml`](../deploy/session-relay/relay.toml) to
   `/etc/iroh-relay/relay.toml`. Set the hostname and contact, and a long
   random `shared_token`. It obtains its certificate from Let's Encrypt.
4. Install [`iroh-relay.service`](../deploy/session-relay/iroh-relay.service)
   and `systemctl enable --now iroh-relay`.
5. Host with `Relay: "https://<name>/"` and `RelayToken: "<token>"`.

Without the token the relay refuses an endpoint, and the session ends
`Unreachable` with "refused access". Invitations carry the token to guests,
so anyone holding an invitation can relay through it while it remains
configured; rotate the token to cut that off. For a LAN or a test,
[`relay-dev.toml`](../deploy/session-relay/relay-dev.toml) runs plain HTTP
with `iroh-relay --dev --config-path relay-dev.toml`.

**Operating it.** Whoever runs the relay pays for it and keeps it up; the
Engine does not run one. A five-player session sending a 4 KB state every two
seconds moves about 8 KB/s, or 30 MB an hour, through the relay. 1,000
session-hours a month is about 30 GB, which the smallest VPS plans (around
€4–6 a month) carry. The work is the domain, the certificate and uptime, not
bandwidth. number 0 also sells hosted relays.

## Runtime and content

- **Dependencies.** Nothing beyond the runtime pack, on Linux and Windows
  alike: QUIC and TLS (rustls with ring) are built into the Engine. A session
  needs outbound UDP, and HTTPS to its relay when it uses one. NativeAOT and
  CoreCLR products use the same service.
- **Offline.** A product that never calls `Host` or `Join` makes no
  connection and needs no configuration; one that plays alone can always do
  so.
- **Content.** The Engine sends no content between players. Every player
  needs the same game and modules; put a version or content identity
  (`Content.ReadBundleIdentity`) in the `Application` tag or in the product's
  first message, and refuse or fetch what differs (the `Http` service can
  download a missing module, see [HTTP downloads](http-downloads.md)).

## Adopting it in a party RPG

A party game keeps its rules where they are and adds one host loop and one
guest loop. A sketch for a Gold Box-style game, where the leader walks, the
party votes in dialogue and each player commands their own character:

```csharp
// Host: the game the party plays. Rules stay in the product.
foreach (SessionEvent e in sessions.TakeEvents(session).Span)
{
    switch (e.Kind)
    {
        case SessionEventKind.MemberJoined or SessionEventKind.MemberRejoined:
            seats.Seat(e.Member, KeyOf(e.Member));        // product: who plays whom
            sessions.SendView(new(session, e.Member, Encode(game.Snapshot())));
            break;
        case SessionEventKind.MemberLeft:
            seats.Away(e.Member);                          // product: who leads now, who is waited for
            break;
        case SessionEventKind.Message:
            PartyAction action = Decode(e.Payload);
            string? refusal = action switch
            {
                Move m when e.Member != party.Leader => "only the leader walks",
                Vote v when v.Phase != dialogue.Phase => "that choice was for an earlier moment",
                Command c when !seats.Commands(e.Member, c.Character) => "not your character",
                _ => game.Apply(e.Member, action),
            };
            if (refusal is not null) sessions.Send(new(session, e.Member, EncodeNotice(refusal)));
            break;
    }
}
if (game.Changed) sessions.Broadcast(new(session, Encode(game.Snapshot())));

// Guest: render the latest state the host sent; send choices, never state.
foreach (SessionEvent e in sessions.TakeEvents(session).Span)
    if (e.Kind is SessionEventKind.View or SessionEventKind.Message)
        view = Decode(e.Payload);
sessions.SendToHost(session, Encode(new Vote(dialogue.Phase, choice)));
```

The host runs the one simulation; guests draw from its states with their own
Engine renderer. Saves, the leader, tie-breaks, turn order and what happens
to an absent player's character are product decisions, and so is whether a
rejoining key reclaims its character. Chat needs no product code beyond
showing `ReadChat` and calling `SendChat`.

## Example

[`fixtures/csharp-multiplayer-party`](../fixtures/csharp-multiplayer-party) is
a small party game. Players host and join, claim characters and chat. The
leader walks a corridor the Engine renders locally for every player, and the
party votes at encounters, with the leader breaking ties. Its rules are in
`Party.cs`; the Engine supplies everything else.
