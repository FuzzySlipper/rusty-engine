# C# first-person clips

Package-consumer fixture for [clip-led arms](../../docs/first-person-held.md#clip-led-arms).
The arms are Drillimpact's PSX first-person arms (CC0 1.0, version 1.1.0,
<https://drillimpact.itch.io/psx-first-person-arms-free>), split by
`make_arms.py` from asset-pipeline's
`tools/fps-arms/packs/psx-drillimpact/arms_rig.glb`:

- `content/arms.glb`: the arms with their resting, guard and idle clips;
- `content/arms.knife.glb`: a clip pack carrying only the arms' skeleton
  and the knife and jab clips, associated through `ArmRig.Packs`.

The look is the joint attachment fixture's box (`weapon.glb`), scaled to a
knife and to a staff. The knife's length leaves the fist on the little
finger's side, as the clips swing it. The arms are placed so the rig's own
`camera` joint sits at the camera, raised 8 cm to frame the strike.

Scenes, each with head bob carrying the arms (`Carry`):

- `strike-quick` and `strike-slow`: the pack's `knife_hit_01`, its phases
  marked in the clip (`ArmClip`), played at two `PhaseTiming`s between
  stretches of `knife_idle`. The knife rides the main hand.
- `two-hand`: the staff over the model's `guard_idle`; the off hand
  reaches the staff's off grip by IK (`HandAim.Reach` 1).
- `weapon`: weapon-led; the arms reach a still knife.
- `cycle` (the default): each in turn, the arms fading between clips and
  modes.

The Engine measures, on the pose it drew (landmark pairs in
`fixture.rides`, shown by `engine.inspect.overlay show`), the main wrist
against the point on the look the hold puts it at, and on the staff the off
hand's palm against the off grip. The product keeps the worst of each per
scene and the look's largest move in one update while the arms fade. A hand
whose wrist is clamped into range is turned about its grip, so while the
arms fade from weapon-led the main wrist can leave its point with the palm
still on the grip.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
rusty dev --project CsharpFirstPersonClips.csproj --engine-source /path/to/rusty-engine --live-debug
```

Live-debug commands:

- `clips.inspect`: the scene, the look's grip and heading, what the arms
  sample (`Clip`), the worst measures per scene, the wrist clamp, the
  shortfall and the pairs as last measured.
- `clips.scene <cycle|strike-quick|strike-slow|weapon|two-hand>` starts a scene.
- `clips.at <seconds>` holds the scene at a time (negative runs it again).
- `clips.bob <true|false>` turns head bob on or off.
- `clips.reset` clears the worsts.
