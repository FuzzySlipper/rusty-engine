# C# fog stir

Package-consumer fixture for [fog regions and stirrers](../../docs/lighting-and-sky.md#fog-regions-and-stirrers).
A mist sump 30 m long, 6 m high and 4 m deep fills an open-sided passage
in front of the camera. A dark body glides along it and back at 3 m/s with
a stirrer on it, so the Engine parts the mist ahead of the body, clears a
wake behind it and lets the wake settle over three seconds. A lantern
sweeps along the front of the passage and lights the mist it passes.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpFogStir.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpFogStir.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `fogstir.observe` is read-only. It reports the mist's mode, the body's
  place and velocity, the drifters, the stirrer publishes and the volumetric
  fog quality in effect.
- `fogstir.quality off|low|high` sets the volumetric fog quality as a
  player's video option does.
- `fogstir.drifters 0..3` adds slow drifters, each with its own stirrer.
- `fogstir.mode region|volume|none` draws the mist as the stirred region,
  as one plain box fog volume with the same values, or not at all. Use it
  to compare costs.
- `fogstir.pause true|false` holds everything still, so the wake settles,
  or lets it move on.
