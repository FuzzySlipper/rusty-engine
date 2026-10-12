# Floating bodies

Ordinary packaged C# product. Dynamics water
([floating bodies](../../docs/rope-physics.md#floating-bodies)): a still lake
and, beyond it, a river flowing at 1 m/s, both flat planes at height 0. In the
lake a 2 × 0.3 × 1.5 m raft of 180 kg, a 0.3 m ball of 40 kg and a capsule of
60 kg (0.5 m half height, 0.2 m radius) that tips over and lies down; in the
river a second raft tethered 6 m downstream of a fixed anchor. `float.load`
puts a 60 kg load's weight 0.8 m off the lake raft's centre (a force and its
moment, submitted each step).

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-floating-bodies/CsharpFloatingBodies.csproj --live-debug
```

Live debug commands:

- `float.inspect`: each body's draft against Archimedes, the raft's roll
  against hydrostatics, the river raft's position and speed, each body's
  water facts, and the mean Dynamics step time.
- `float.load <true|false>`: put the load on the lake raft or take it off.
- `float.expect <stage>`: assert a stage of the headless sequence.
- `float.cost <bodies>`: microseconds of water per floating body per step,
  timed around the generated `Step` for that many balls in a separate world,
  dry and then floating.

## Headless sequence

Run headless with live debug, hold time with `engine.time.mode manual`, and
advance with `engine.time.advance 2000` repeated.

1. Advance 20 s, `float.expect drafts`: the raft, ball and lying capsule float
   within 1 cm of their Archimedes drafts. `float.expect tethered`: the river
   carried its raft to the tether's length and the tether holds it.
2. `float.load true`, advance 10 s, `float.expect tipped`: the raft rolls
   toward the load within 25% of the roll its metacentric height gives.
3. `float.load false`, advance 16 s, `float.expect righted`: it is level again
   within 0.01 rad.
