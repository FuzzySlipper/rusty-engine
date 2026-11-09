# Parts rigs

Package-consumer fixture for [parts rigs](../../docs/parts-rigs.md).

- A townsperson walks in place: four views of rusty-dagger's prototype rigs
  (`content/people/townsperson`: front, side and back, and the side again
  mirrored). The Engine draws the view each camera sees; all four walk on one
  clip time. Over the walk on track 0, the rig's `idle` (its breathing,
  head and hair sway) is added on track 1.
- Beside it, rig-c's gallery `features` rig (MIT, `content/people/features`,
  the same rig as `animation-parts-rig`'s fixture) loops clip `a`, which
  fires `step` and `cue` events, with clip `b` added over it on track 1 at
  alpha 0.6.
- On the other side, rig-c's gallery `look` (MIT, `content/people/look`, the
  same rig as `animation-parts-rig`'s fixture) turns its face because a
  value says so: its `yaw` slider reads the `yaw_dial` bone's rotation as a
  time in clip `turn`, and the product holds the dial with `DriveBone`.

The model documents are rig-c's `skeleton.model.json` with whitespace
removed, beside the pages they name.

```bash
RustyEngineFixtureSdkVersion=<version> RestoreAdditionalProjectSources=<feed> \
  rusty dev --project fixtures/csharp-parts-rigs/CsharpPartsRigs.csproj --live-debug
```

Live-debug commands:

- `rigs.observe` is read-only. It reports each rig's tracks 0 to 2 (clip,
  clip time, looping, alpha, additive), speed and views; the camera's bearing and the view it draws; the attached bone
  in that view and where the camera draws it, beside where the marker was
  published; the held `turn` value; and the events read so far.
- `rigs.play <clip> <crossfade seconds> <speed>` plays `walk` or `idle` on
  the townsperson's base track, looping.
- `rigs.stop <crossfade seconds>` fades its base track back to its setup
  pose; the layers above play on.
- `rigs.layer <rig> <track> <clip> <alpha> <additive> <crossfade seconds>`
  loops a clip on a track of `person` or `features` over the tracks below:
  `additive` true adds its keyed values, false replaces them by `alpha`.
  `rigs.alpha <rig> <track> <alpha>` changes a track's alpha while it plays;
  `rigs.unlayer <rig> <track> <crossfade seconds>` fades it out.
- `rigs.turn <value>` holds its `turn` bone's x (about -24 to 24 turns the
  head; only the front rig has the bone); `rigs.release` lets go.
- `rigs.yaw <degrees>` holds the look rig's `yaw_dial` rotation (-19 to 19
  degrees); the face turns to match. `rigs.observe`'s `look` reports the held
  value, the dial's rotation and the rig position of its `nose`.
  `rigs.yaw.release` lets go.
- `rigs.orbit <degrees>` puts the camera at a bearing about it, from in
  front toward its left; `rigs.spin <degrees per second>` orbits it (0
  stops). Front is 0, the drawn side -90, the back 180 and the mirrored side
  90.
- A red marker stays on a bone of the townsperson: each update the product
  reads the bone for its camera (`ReadBone`) and publishes the marker where
  that camera draws it. `rigs.attach <bone>` picks a bone every view has
  (`head` by default, `neck`, `foot_r`, `leg_l1`, ...); `rigs.place <x> <z>
  <yaw degrees>` moves and turns the townsperson.
