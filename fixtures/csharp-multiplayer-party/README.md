# Multiplayer party proof

A small social party game over the Engine `Session` service
([multiplayer sessions](../../docs/multiplayer-sessions.md)). Build it with an
explicit `RustyEngineFixtureSdkVersion` and `RestoreAdditionalProjectSources`
for the matching SDK feed. Run one copy per player, for example
`rusty dev start --instance host`, `--instance alice` and `--instance bob`.

Everything is played from the panel:

- **Host a party** with a name and a relay: `n0` (public, for development),
  direct only for a LAN, or a relay URL and token you run
  (`deploy/session-relay`). Copy the invitation it shows.
- **Join** with your own name and the pasted invitation. The name is also the
  Engine identity, so rejoining under the same name keeps your member number
  and character.
- Claim a character; chat at any time.
- The **leader** (at first the host) turns and walks the corridor, which every
  player's Engine draws from the host's state. The leader can pass leadership
  to any present member; while the leader is away, the host leads.
- The blue marker is a **vote**: everyone votes, and the leader breaks a tie.
  The red one is a **fight**: each player with a character picks an action.
  The party waits for absent players until the leader chooses **Decide without
  them**. Buttons drawn before the party moved on are refused, not counted.
- **Leave**, rejoin, or close the host's game: guests see why their session
  ended.

The members list shows each connection's path (`Direct` or `Relay`) and
round-trip time. `party.inspect` reports one player's session, members, chat
and party state. `party.do <json>` runs one panel action through the same
product method as the buttons, as assistance for agents; JSON quotes need
backslash escapes for the debug command line.
