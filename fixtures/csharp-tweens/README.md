# C# tweens

Package-consumer fixture for [appearance tweens](../../docs/appearance-tweens.md).
An orange piece hops around four cells of a board. The product moves it one
cell at a time and publishes it once per move. The Engine plays the hop with
its arc and the landing squash (`Tweens.HopFrom`), and reports a marker at
the apex and the hop's completion. On each completion the product starts a
punch and a flash on the blue beacon, then waits 0.35 s of world time before
the next move. The teal piece breathes forever on a layered, yoyo tween.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpTweens.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpTweens.csproj --runtime /path/to/pair/runtime-pack
```

Live-debug commands:

- `tweens.observe` is read-only. It reports the cell, the hops started and
  completed, the apex markers, the product's publishes (one per move), and
  the hop and breath readouts.
- `tweens.skip` completes the running hop, as an input skip would. Its
  completion arrives next update.
- `tweens.retarget` moves the piece on to the next cell mid-hop. The new hop
  starts from where the piece shows.
