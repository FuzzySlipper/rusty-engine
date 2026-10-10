# Appearance tweens

`IEngineContext.Tween` plays eased motion over published appearance objects:
hops, squash and stretch, punches, shakes, fades, flashes and looping pulses
for sprites, primitives and meshes. The product publishes where an object
really is, as usual and immediately. The Engine shows a presentation offset
over it until the tween ends, with no republish while it plays. Gameplay
state never waits for presentation.

```csharp
// The piece moves now; the Engine shows the hop from where it was.
engine.Graphics.PublishChanges(new(new[] { pieceFact }, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<MeshJointAttachment>.Empty));
TweenHandle hop = engine.Tween.Start(Tweens.HopFrom(pieceId, previousCell - newCell, height: 0.8f, seconds: 0.45f, squash: 0.25f)).Tween;

// Later updates: sequence from its completion, or let input skip it.
foreach (TweenEvent e in engine.Tween.ReadEvents().Span)
    if (e.Tween == hop && e.Kind == TweenEventKind.Completed) StartNextMove();
engine.Tween.Control(new(hop, TweenControl.Complete));
```

See `fixtures/csharp-tweens` for a board piece that hops cell to cell, a
breathing piece, and a beacon that punches and flashes when each hop lands.

## Timelines

`Start` takes a `TweenStartRequest`: an object, a flat array of
`TweenSegment`s, optional `TweenMarker`s, iterations (or `Forever`), `Yoyo`,
a clock and a start mode. Each segment has a start time, duration, channel,
layer, shape, easing and values. Sequence, overlap and delay come from the
start times; the iteration ends with the last segment.

| Channel | Values | Shows |
| --- | --- | --- |
| `Translation` | `xyz` | added to the published translation, in parent space |
| `Rotation` | `xyzw` quaternion | applied in the object's local frame |
| `Scale` | `xyz` | multiplying the published scale |
| `Tint` | RGBA | multiplying a sprite's tint or a primitive's colour, kept within [0, 1] |

A channel's **base** segments form one track: it shows the latest base
segment that has started, holds its end value until the next starts, and
shows its first segment's start before that. An **additive** segment adds
nothing before it starts and holds its end value afterwards. Translations add
and the other channels multiply. So a delayed base segment shows its start
from the beginning, which suits a delayed fade in; a landing squash is either
additive or follows a base segment that holds unit scale in flight, as
`Tweens.HopFrom` does. A squash after a hop, or a punch over a
moving piece, is two segments on one timeline.

Shapes:

- `Tween`: `From` to `To` by the easing. On translation, `Arc` is added at
  `4p(1 - p)` of linear progress `p`: a parabolic hop that peaks half way
  whatever the horizontal easing.
- `Punch`: swings from `From` toward `To` and past it `Frequency` times, the
  swing shrinking to nothing by the easing.
- `Shake`: smooth noise around `From`, each component reaching up to
  `To - From`, at `Frequency` changes per second, fading by the easing.
  `Seed` selects the noise, so a shake repeats exactly.
- `Spline`: a Catmull-Rom span from `From` to `To` shaped by `Before` and
  `After`. `TweenSegment.Path` builds spans through a list of points.

Easings are linear; quad, cubic, quart, quint, sine, expo, circ, back,
elastic and bounce, each in, out and in-out; CSS `cubic-bezier`; `steps(n)`;
and a damped spring (`TweenEasing.Spring(stiffness, damping)`). The spring's
whole settle time, to within 0.1%, is stretched over the segment, so its
shape does not depend on the duration. Back, elastic and an underdamped
spring overshoot; the others stay within the segment's values. The curves
live in `render-presentation`; products do not reimplement them. Presentation
code that needs a curve's value asks the Engine:
`engine.Tween.Evaluate(TweenEasingKind.BackOut, progress)` returns what a
segment with that easing plays at linear progress `progress`, and
`engine.Tween.Sample(timeline, seconds)` returns the offset (translation,
rotation, scale and tint) a start request's timeline shows that long after it
starts, such as the pose to place a muzzle flash at. Neither needs a
published object or starts anything.

`Yoyo` plays every second iteration backwards. A marker is reported each
iteration the timeline passes its time, backwards ones included, so it fires
when the pose passes it; a marker where a yoyo turns is reported once. A
marker at 0 is reported by the first update.

`Tweens` builds common timelines: `HopFrom`, `Breathe`, `PunchScale`,
`Shake`, `Flash` and `FadeIn`. Each returns an ordinary request for `with`
changes. `TweenSegment` has `Move`, `Hop`, `Rotate`, `Scale`, `Tint`,
`Punch`, `Shake` and `Path` builders, with `At(start)` and `Additive()`.

## Starting, ending and events

- A tween shows its start at the end of the call that starts it. It advances
  at the start of each update, before the product runs, and the end of every
  call writes what each changed tweened object shows, after the call's own
  publishes.
- `Replace` ends the object's tweens and plays the new one as given.
  `FromPresented` ends them too, and each channel's first base tween or spline
  segment starts from the pose and tint shown: a move that replaces a hop
  mid-flight continues without a jump. With no tween running it plays as
  given. `Layer` plays alongside the object's tweens; their offsets compose in
  start order.
- `ElapsedSeconds` on the request starts the tween as if that much time had
  already passed: it shows the pose an update advancing by that time would
  show, and reports only the markers after it. An action already part way
  along when the product publishes starts where it is.
- A republish moves the published values under a running tween; the offset
  keeps showing over the new values.
- When a tween ends, its offset stops showing: the object shows its published
  values from the end of that call. Author timelines that end at the channel's
  identity (zero translation, no rotation, unit scale, white), as the
  builders do; a fade out that should stay invisible ends by the product
  hiding or removing the object when it completes.
- `ReadEvents` returns this update's events, markers crossed and tweens
  completed, tween by tween in start order and each tween's in time order. `Control` pauses, resumes, completes, cancels
  or seeks. `TweenControlRequest.Seek(tween, seconds)` moves to that time
  after the start, clamped to the timeline, and the call shows that pose; a
  paused tween stays paused there, so a viewer can jog and step a timeline.
  A seek reports no markers it jumps over. A playing tween sought to its end
  completes at the next update.
  Complete jumps to the end, ends with the call, and is reported as completed
  by the next update. A tween that plays forever stops where it is. Cancel
  ends it with no event. Removing the object ends its tweens with no event.
- `Read` and `Control` return a `TweenReadout`: state, elapsed seconds,
  iteration length, total seconds (infinite when it plays forever) and
  iteration. An ended tween reads as `Ended` and needs no release.
- `Start` refuses an object that is not in the published scene
  (`CSHARP_TWEEN_OBJECT`), invalid times or values (`CSHARP_TWEEN_DEFINITION`),
  and a tint on an object with no colour to change, such as a mesh
  (`CSHARP_TWEEN_TINT`).

## Clocks

`World` (the default) follows world time: the admitted steps, and between
them the world time already owed toward the next step, so it holds and slows
with [gameplay time](csharp-lifecycle.md#gameplay-time) and never runs ahead
of the steps by more than one. `Realtime` follows host time, so hover and menu
motion keep moving while the world is held. Neither moves during a lifecycle
pause.

Tweens move at the display's frame rate, not only at steps. Host
observations come at the fixed-step rate; while a tween plays and the display
is faster (a page is taken to show 60 frames a second, a desktop window
reports its monitor's refresh), the host also presents between them. Those
presentations, and observations without a step for a product that did not
select gameplay time, advance and show the tweens without calling the
product, and the next observation counts only the time they have not shown.
A product that selected gameplay time still updates at every observation.
Markers and completions crossed between updates are reported by the next
update.

## Limits

- Tweens are sampled at each presentation. The rest of world
  presentation (particles, sprite playback, GLB animation) still moves at
  steps.
- Spatial picking, rays, collision and gameplay use the product's own
  transforms, not the offset shown, as with
  [camera samples](csharp-lifecycle.md#retained-camera-composition). Particle
  and audio anchors on an object's entity follow what is shown.
- Tint applies to sprites and primitives. Mesh colour factors belong to the
  appearance, which other objects may share, so a mesh has no tint channel.
- DOM product UI is out of scope: CSS transitions and the Web Animations API
  already animate it. Tweens are for what the viewport draws.

## Cost

Each presentation while a tween plays costs one timeline sample per
active tween (a few segments each)
and one transform update, plus a colour update for a tint, per object whose
shown values changed; presentations between steps (and, for a product that
did not select gameplay time, observations without a step) add one Engine
call of their own. Nothing is presented between observations while no tween
plays. While any tween
runs, the end of each call also passes
over the render updates that call made, to find tweened objects the product
republished. Objects without tweens are not examined. A call with no tweens
does nothing. Measured in a release build of the services bridge, an update with
hopping objects costs about 25 µs for 100 tweens and 0.5 ms for 1,000, and
nothing measurable with none.
