# Parts rigs

Package-consumer fixture for [parts rigs](../../docs/parts-rigs.md).

- A townsperson walks in place: four views of rusty-dagger's prototype rigs
  (`content/people/townsperson`: front, side and back, and the side again
  mirrored). The Engine draws the view each camera sees; all four walk on one
  clip time. Over the walk on track 0, the rig's `idle` (its breathing,
  head and hair sway) is added on track 1.
  Her material casts (`SpriteShadowPolicy.CastAndReceive`): a low sun at her
  front left throws her walking figure across the floor, from the view
  nearest the sun's bearing, whichever view the camera sees.
- Beside it, rig-c's gallery `features` rig (MIT, `content/people/features`,
  the same rig as `animation-parts-rig`'s fixture) loops clip `a`, which
  fires `step` and `cue` events, with clip `b` added over it on track 1 at
  alpha 0.6.
- On the other side, rig-c's gallery `look` (MIT, `content/people/look`, the
  same rig as `animation-parts-rig`'s fixture) turns its face because a
  value says so: its `yaw` slider reads the `yaw_dial` bone's rotation as a
  time in clip `turn`, and the product holds the dial with `DriveBone`.

- In the lower left, rig-c's gallery `portrait` (MIT, `content/people/portrait`,
  the same rig as `animation-parts-rig`'s fixture) is a dialogue bust: the
  UI draws a frame and anchors its window as `bust`, and the product places
  the rig with `SetViewport` and `SetViewportAnchor`, fitting a 127-texel
  square of the rig (`TexelSnap` 640/127 units). The page's layout moves and
  sizes it on each resize with no product call: the window is 127 CSS pixels
  times the largest whole scale up to 3 that the window allows. It loops
  `idle` (breathing and blinks) in `RenderLayer.Viewmodel`, published far off
  and turned to show the transform is not read. It draws after the scene's
  finish, so its pages' colours reach the screen unexposed and ungraded.

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
- `rigs.bust.play <clip> <crossfade seconds>` plays `idle`, `gaze` or
  `turn` on the bust; `rigs.layer bust ...` layers over it. `rigs.bust.frame
  square|setup` fits the 127-texel square or the setup pose's bounds (the
  whole plate). `rigs.bust.anchor false` places it by the product's own
  square, computed from the window's CSS size and placed again on each
  resize; `true` follows the UI's window again. `rigs.fog <density>` sets exponential fog over the scene,
  which the bust does not take; `rigs.finish <exposure>` finishes the scene
  as a lit one is (ACES at that exposure, graded, bloomed, auto exposed up
  to 4; 0 for none), and the bust still shows its pages' colours. `rigs.observe` reports the bust's tracks and
  its placement (the product's square: CSS pixels a texel, side, window
  size, how many times it placed it, and whether it is anchored).
- `rigs.orbit <degrees>` puts the camera at a bearing about it, from in
  front toward its left; `rigs.spin <degrees per second>` orbits it (0
  stops). Front is 0, the drawn side -90, the back 180 and the mirrored side
  90.
- A red marker stays on a bone of the townsperson: each update the product
  reads the bone for its camera (`ReadBone`) and publishes the marker where
  that camera draws it. `rigs.attach <bone>` picks a bone every view has
  (`head` by default, `neck`, `foot_r`, `leg_l1`, ...); `rigs.place <x> <z>
  <yaw degrees>` moves and turns the townsperson.
