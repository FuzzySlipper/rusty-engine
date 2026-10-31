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

A `FirstPersonArms` draws one arms model by its `ArmRig` profile. It opens
`ArmRig.Model` from the product's own content; a product whose arms live in a
`ProductContentBundle` it loaded (operator-local art kept out of its release)
opens the model there and passes the `ContentReference` instead, and the
rig's `Model` then only names it in errors. The profile gives:

- the GLB and the clip played under the IK;
- the model's turn and scale;
- each arm's joints by name, with the forearm's twist joints (`Twist`);
- `DigitFrames`, which says how the digits turn;
- the palm's offset from the hand joint;
- a per-hand `Grip` that brings the clip's hands to the arms' canonical
  holding pose;
- the wrist's range (`Wrist`).

`ArmJoints.Twist` lists forearm joints that share the hand's roll, each with
its share (0 to 1; 0.5 is typical): a twist joint beside the hand (UE's
`lowerarm_twist_01`) or a forearm segment between the lower arm and the hand
(a CAT rig's second forearm bone). Each update they turn by their share of
the hand's roll about the forearm, as the Engine draws it after the reach
([twists](skeleton-poses.md#twists)), and the hand keeps its turn, so a
rolled hold doesn't pinch the wrist. A listed joint must descend from the
upper arm and not from the hand; the arms refuse one that doesn't, naming
`main.twist[i]` or `off.twist[i]`.

Each hand's frame is measured from its knuckles into a canonical hand frame:
+Y along the fingers, +Z out of the back, +X toward the little finger on
the right hand. Holds, per-look `HandShape`s and closure therefore mean the
same hand on every rig, and a new arms mesh is a new profile, not new
holds.

Fingers curl about their own X (`DigitFrames.Native`) or about the measured
knuckle line (`Measured`). A thumb is aimed on either: each bone is turned to
where its clip's `[curl, swing]` plus the added ones read in the canonical
frame (`HandKnuckles`), so a thumb shape lands as written whatever the
thumb joints' own frames.

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

### Wrist range

A hand is oriented after the reach, so a hold that disagrees with the
forearm the IK draws would bend the wrist past what a wrist does.
`ArmRig.Wrist` (a `WristRange`, degrees) bounds it: `flexion` toward the
palm (70 by default), `extension` toward the back of the hand (60),
`radial` toward the thumb (20), `ulnar` toward the little finger (35), and
`roll` about the forearm either way (90), which the twist joints share. The
roll is from the hand's neutral turn: the thumb toward the inside of the
elbow (away from its pole) and the back of the hand outward. Each reach
works out the forearm and elbow bend the Engine's solve will draw for the
wrist's target and pole, and measures the asked hand against them. A hand
past the range first has its elbow swing about the line from the shoulder
to the wrist, up to `elbow` degrees (30) from its pole, when that brings
the forearm under the hand or takes up its roll. The rest is clamped: the
hand is turned the least that brings its bend to the range's edge, then
rolled back about the forearm. A clamped hand turns about its grip, so its
palm stays on what it holds and the wrist moves.

`WristClamp` reports each hand's last reach as a `WristBend`: the asked
flexion (negative is extension), ulnar deviation (negative is radial) and
roll (positive turns the palm up) with the elbow at its pole, how far the
hand was turned back (`Clamped`, 0 in range) and how far the elbow swung. A debug command can
list the looks whose holds ask past the range.

### Solved grips

A `SolvedGrip` is a grip found by contact around the look, such as
asset-pipeline's `tools/fps-arms/grip.py` writes. Give it to a hand as
`HandAim.Solved`, with `HandAim.Grip` at the look's grip point (the point
the solve held it at). The hand is then turned and placed so the look sits
in it where the solve put it, in place of a hold and the rig's palm offset.
Its shape applies when the aim has none, over the rig's `Grip` as a look's
shape does. The format is JSON:

```json
{
  "shape": {"fingers": [[8, -30, -17], [19, -30, -16], [23, -62, -39], [14, -61, -39]],
            "thumb": [[47, -6], [-54, 58], [-10, 67]]},
  "object": {"in_canonical": {"position": [0.0, 0.0743, -0.0294],
                              "rotation": [0.43046, -0.43046, 0.56099, -0.56099]}}
}
```

- `shape`: a `HandShape` for the right hand, over the canonical holding
  pose (`ArmDigits.Pose`).
- `object.in_canonical`: the look's placement in the right hand's
  canonical frame (+Y along the fingers, +Z out of the back, +X toward the
  little finger), from the hand joint. `position` is where the grip point
  sits (metres) and `rotation` the quaternion `[w, x, y, z]` that takes the
  look's axes (its length along -Z, its up side +Y) into the hand's.

Other members may sit beside these, as `grip.py`'s diagnostics do; a
product reading the file with unmapped members refused reads these with
its own options. The placement is of the hand the grip was solved on, so a
rig whose palm sits elsewhere from its hand joint needs its own solve. On
the off hand, a solved grip mirrors as a hold does.

## Clip-led arms

The arms above are weapon-led: only the weapons move by authored motion,
and the arms follow by IK. The same `FirstPersonArms` can instead be
clip-led: baked arm clips (authored in Blender on a control rig, retargeted
mocap, a clip pack) drive both arms, and the look rides the main hand. The
elbows, shoulders, arcs and follow-through are then the clip's, not the
IK's. A product picks the mode per look or per action, each update.

Pick weapon-led for looks with no clips of their own, motion that must suit
every rig, and holds tuned against the hand. Pick clip-led when the motion
is authored on the arms: a strike whose shoulder and elbow act, a clip pack
that ships its clips, or a pose IK keeps getting wrong.

### Clips and timing

The rig plays clips from its GLB and from `ArmRig.Packs`, clip packs on the
same skeleton ([clip packs](portable-assets.md#animation-clips-and-clip-packs)),
each with the producer and licence its association records. `Clips` lists
them. A product whose packs live in a content bundle passes their
`ContentReference`s to the constructor beside the model's.

An `ArmClip` is a clip an action plays, with `ClipMarks`: where its windup
starts, its strike lands (the commit starts), its commit ends and its
recovery ends, as fractions of the clip. `ArmClip.At(timing, seconds)` fits
each phase of the clip to the action's `PhaseTiming`, so the clip plays
through the windup over the product's windup seconds, and so on. Gameplay
timing stays in product content: retuning a weapon's speed needs no new
export. `Fade` (seconds, 0.15 by default) is how long it fades in.

```json
{"clip": "knife_jab", "marks": {"start": 0.05, "strike": 0.35, "recover": 0.5, "end": 0.95}, "fade": 0.1}
```

### Each update

1. Call `Lead` with the clip frame, `arm.At(timing, actionSeconds)`, or
   null for weapon-led, and the seconds since the last update on the clock
   the action runs on. An idle or guard loop is an `ArmClipFrame` the
   product times itself.
2. Set `Lean` and `Carry` (below).
3. Call `Ride` with the main hand's aim. It samples the clip pose and
   returns where the look is held, its grip point and turn, by the aim's
   solved grip or hold and the rig's palm, as a weapon-led hand holds it.
   Publish the look there. A look held in both hands places its off grip
   from it.
4. Publish `Fact(shown)`, then call `Reach` with the same aims. Skip
   `Aim`: a clip-led main hand reaches for nothing.

The main hand is the clip's: no IK, its digits as the clip shapes them.
The off hand plays the clip too, or leaves it for its grip by its aim's
`Reach` (0 the clip, 1 the grip), which a curve can fade. The grip of a
look held in both hands is reached by IK after the clip, with the off
hand's shape and closure, as a weapon-led hand's is. Weapon-led hands
always reach.

### Fades

A new clip fades in over the clip before, which holds where it was. A clip
started inside another's fade fades in over whichever of the two showed
more; the other drops out.
The arms fade between clip-led and weapon-led over the clip's fade, for
example while a weapon is drawn. In between, the look's placement blends
from the aim's to the ridden one, and the main hand reaches the blend, so
neither the look nor the hand jumps. The arms stay placed where they were
last drawn weapon-led, so a clip's shoulder acting shows.

### Procedural layers

Head bob, sway, landing, settle and `HeldLayers` punches move the arms by
`Carry` (camera space metres) and `CarryTurn` (about the shoulder). The
ridden look moves with the hand that holds it. Weapon-led hands keep their
grips under a carry. Give a weapon-led look the same offsets as today and
`Carry` too, and the two modes move alike.

### Wrist range, twist and readouts

A clip that bends the main wrist past `ArmRig.Wrist` is held in range as a
weapon-led reach is: the elbow swings first, then the hand is turned back,
with the IK solving the arm to the swung elbow. The ridden look follows the
kept hand. Twist joints share whatever roll IK and overrides add on top of
the clip, so a clip that animates its own twist joints keeps them.

`Clip` reports what is sampled: the clip, time and phase, any clip fading
out under it, the fade, and how far the arms are clip-led (`Led`).
`WristClamp`, `Knuckles`, `Shortfall` and the hand rigs for
`GripLandmarks` read as they do weapon-led.

### Limits

- One skeleton per clip set. Clips are not retargeted between skeletons.
- Clips should animate every arm joint. The look is placed from a sample of
  the clips alone, and a joint a clip lacks shows the rig's clip below it.
- Additive clip layers on top would move the drawn hand off the ridden
  look. Use `Carry` for procedural motion.
- The digits of a clip-led main hand are the clip's. A look's shape and
  closure apply to the off hand that reaches.
