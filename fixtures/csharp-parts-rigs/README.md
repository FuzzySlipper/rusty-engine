# Parts rigs

Package-consumer fixture for [parts rigs](../../docs/parts-rigs.md).

- A townsperson walks in place: four views of rusty-dagger's prototype rigs
  (`content/people/townsperson`: front, side and back, and the side again
  mirrored). The Engine draws the view each camera sees; all four walk on one
  clip time.
- Beside it, rig-c's gallery `features` rig (MIT, `content/people/features`,
  the same rig as `animation-parts-rig`'s fixture) loops clip `a`, which
  fires `step` and `cue` events.

The model documents are rig-c's `skeleton.model.json` with whitespace
removed, beside the pages they name.

```bash
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-parts-rigs/CsharpPartsRigs.csproj --live-debug
```

Live-debug commands:

- `rigs.observe` is read-only. It reports each rig's clip, clip time, speed
  and views; the camera's bearing and the view it sees; the townsperson's
  `head` bone in that view; the held `turn` value; and the events read so
  far.
- `rigs.play <clip> <crossfade seconds> <speed>` plays `walk` or `idle` on
  the townsperson, looping.
- `rigs.stop <crossfade seconds>` fades it back to its setup pose.
- `rigs.turn <value>` holds its `turn` bone's x (about -24 to 24 turns the
  head; only the front rig has the bone); `rigs.release` lets go.
- `rigs.orbit <degrees>` puts the camera at a bearing about it, from in
  front toward its left; `rigs.spin <degrees per second>` orbits it (0
  stops). Front is 0, the drawn side -90, the back 180 and the mirrored side
  90.
