# First-person held weapons and arms

`Rusty.Engine.FirstPerson` is an optional managed helper for first-person
games that draw a weapon in the hands. It composes Engine mechanisms the
product already has: [appearance tweens](appearance-tweens.md) play the
motion, [skeleton poses](skeleton-poses.md) reach the arms, and hand
landmarks check the grip. It holds the format the motion is authored in and
the arithmetic that turns it into tweens and poses. The product keeps its
content, which weapons show which looks, when actions start, and how hands
swap what they hold.

## Frames

Held things live in camera space: metres right (+X), up (+Y) and back
(+Z), with the view looking down -Z. They are published on
`RenderLayer.Viewmodel`. An authored turn is degrees about X, Y and Z
(`HeldFrames.Turn`: yaw about Y, then pitch about X, then roll about Z).
The off (left) hand holds what the main hand would, mirrored across the
view's centre line (`HeldFrames.Mirror`).

A look is seated on a `HeldStance`: where the grip sits, a point its length
heads toward, and which way its up side faces. Every look of a kind then
sits alike, and a look's nudge corrects it from there.

## Motion

A `HeldMotion` is one family of motion:

- **Poses:** named poses, including `rest`, which is no change. Each pose
  moves the grip and turns the look about it.
- **Tracks:** keyed tracks by action id, with `default` for every other
  action. A `MotionKey` reaches a pose at a fraction of one action phase
  (windup, commit, recovery) and eases in by a named curve. It can also
  close the hand (`grip`), lean the shoulders, bend an elbow toward a point,
  and give the off hand's closure and elbow for a look held in both hands.
- **Loops:** optional `idle`, `walk` and `sprint` sway loops. Each is a
  closed Catmull-Rom curve through offsets. Idle cycles over seconds; walk
  and sprint cycle over metres walked, so the bob keeps step at any pace.
- **Transitions:** optional `equip` and `unequip`, keyed through the poses
  in seconds.

`HeldTimelines` turns these into tween segments that are offsets from rest:

- `Action` covers a whole action at its `PhaseTiming`, scaled and sped up,
  continuing from whatever offset is showing.
- `Transition` draws a look or puts it away, holding the put-away pose past
  its end so the old look never flashes at rest.
- `Settle` eases back to rest when an action is cut short.

Start the result on the look's object and publish the look at rest. To
read what the keys say at the moment the tween has reached (closure, lean,
elbow), use `HeldTimelines.Keyed`.

```csharp
HeldPlacement place = pose => Drawn(look, pose, offHand); // product: stance seat, pose, model fit
TweenSegment[] swing = HeldTimelines.Action(motion, motion.Track(action)!, timing, place,
    TweenEasingKind.QuadOut, scale: actionTime, from: hand.Offset());
engine.Tween.Start(new TweenStartRequest(lookObject, swing) { Clock = TweenClock.World });
```

Each record has a `Problems()` method that lists bad references and
out-of-range values by field. The product reports them against its own
file. The records are plain JSON-friendly types. A product that deserializes
them gives `Vector3` a converter (three numbers, `[x, y, z]`). Properties
are settable, so live tuning edits them in place.

## Feedback and sway

- **Feedback:** `HeldLayers` starts each `HeldPunch` (a damped move and
  turn) as a layered tween on the hand's object, so it adds to the swing.
  Its `Offset()` is the punches' sum now. Add it to the main tween's sample,
  so the arms reach for the weapon as it is drawn.
- **Sway:** `HeldSway` blends a look's loops by the walker's speed and
  whether it is on the ground. It turns a hand's sway down while that
  hand's own action plays (`SwayTuning.Acting`). Publish its `Pose` as the
  look's rest placement.
- **Head bob:** `HeadBob` is the camera's counterpart. It steps by ground
  walked in walking and sprinting gaits, dips on landing, and returns an
  explicit camera basis with a small roll. Aim from the steady eye, not
  the bobbed one.

Advance sway and bob on host seconds. They are presentation, so they keep
moving while gameplay time holds.

## Arms

A `FirstPersonArms` draws one arms model by its `ArmRig` profile:

- the GLB and the clip played under the IK;
- the model's turn and scale;
- each arm's joints by name;
- `DigitFrames`, which says how the digits turn;
- the palm's offset from the hand joint;
- a per-hand `Grip` that brings the clip's hands to the arms' canonical
  holding pose.

Each hand's frame is measured from its knuckles into a canonical hand frame:
+Y along the fingers, +Z out of the back, +X toward the little finger on
the right hand. Holds, per-look `HandShape`s and closure therefore mean the
same hand on every rig, and a new arms mesh is a new profile, not new
holds.

Each update, in this order:

1. Set `Lean` to the motion's lean.
2. Call `Aim` with both hands' `HandAim`s (grip, look turn, hold, closure,
   pole, shape). It leans the body in further when a grip lies beyond the
   arm's reach.
3. Publish `Fact(shown)` in the snapshot.
4. Once that snapshot is published, call `Reach` with the same aims. It
   sets two-bone IK to each wrist, orients each hand, and curls the digits.

`Shortfall` reports how far each wrist fell short. `Knuckles` reads a
hand's angles on its clip and as drawn. To calibrate a rig, set
`HandKnuckles.Difference(pose, clip)` as its `Grip`.

`GripLandmarks` publishes landmark pairs between the drawn hands and a
look's `HeldLandmarks` (trigger, back strap, butt, fore-end), with
tolerances from `HandLandmarkTuning`. The renderer measures each pair on the
pose it drew. `GripLandmarks.Line` formats one measurement for a debug
command.
