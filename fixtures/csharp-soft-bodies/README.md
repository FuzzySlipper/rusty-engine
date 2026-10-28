# Soft bodies

Ordinary packaged C# product for Dynamics
[soft bodies](../../docs/soft-bodies.md). On a floor:

- a 2 m × 1.2 m flag of 21 × 13 particles, its hoist pinned beside a pole,
  in wind along +z whose speed the product varies in a slow gust and a quick
  flutter (7 m/s mean, drag 0.6), with three extra substeps. A held ball
  stands downwind of the pole in its way. The flag is drawn two-sided with a
  textured material (`content/flag.png`);
- a 0.7 m jelly cube of 6 particles a side, 20 kg, corotational at 6 kPa,
  keeping its volume;
- a 0.4 m blob: a shell refined twice, 5 kg, keeping its volume.

Stage with the exact SDK version and matching feed and runtime:

```sh
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-soft-bodies/CsharpSoftBodies.csproj --live-debug
```

Live debug commands:

- `soft.inspect`: each body's particles, pieces, centre of mass, bounds,
  volume against rest, largest stretch, contacts and sleep; the flag's swing
  over the last second, its closest approach to the held ball and most
  contacts; the jelly's height, rest height and lowest height since the last
  drop or squash; the blob's rest and highest centre; the wind; the mean
  Dynamics step time.
- `soft.wind <m/s>`: the wind's mean speed (0 for calm).
- `soft.drop` drops a 30 kg ball onto the jelly; `soft.clear` takes it away.
- `soft.squash` presses the jelly's top down at 8 m/s, fading to nothing
  0.8 m from its top's centre.
- `soft.jump` kicks the blob up at 5 m/s.
- `soft.expect <stage>`: assert a stage of the headless sequence.
- `soft.cost <columns>`: microseconds per step of a `columns` × `columns`
  flag pinned along one edge in steady wind, in a fresh world, against the
  same world empty.

## Headless sequence

Run headless with live debug, hold time with `engine.time.mode manual`, and
advance with `engine.time.advance 2000` repeated.

1. Advance 6 s, `soft.expect flutter`: the flag is held downwind, still
   moving, and stretched little. `soft.expect touched`: it met the held ball
   without passing into it.
2. `soft.drop`, advance 2 s, `soft.expect squashed`; `soft.clear`, advance
   4 s, `soft.expect recovered`.
3. `soft.squash`, advance 1 s, `soft.expect squashed`; advance 4 s,
   `soft.expect recovered`.
4. `soft.jump`, advance 2 s, `soft.expect jumped`.
