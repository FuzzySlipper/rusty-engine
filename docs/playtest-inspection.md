# Playtest inspection

The runtime owns simulation steps. Inspection time modes gate its realtime
scheduler; a manual advance keeps the product's fixed-step cadence and uses the
ordinary C# update/input path. Admitted updates keep the configured fixed
delta. Switching back to realtime clears the wall-time
baseline. Inspection overrides a product's
[gameplay time](csharp-lifecycle.md#gameplay-time): a held mode stops it, a
manual advance ignores its rate, and switching back continues it. Holding does not stop input admission or renderer inspection.
While held, the host still queues posted input; each live-debug command hands
the queued input to the runtime before it runs. So a key pressed before
`engine.time.advance` is held during the steps it admits.

Native live-debug commands (require the ordinary live-debug opt-in):

- `engine.time`: effective mode, step count, cadence and held status.
- `engine.time.mode realtime|manual|action-driven`.
- `engine.time.advance MS`: positive duration up to 2000 ms, rounded up to fixed
  steps, returning actual advancement. Both held modes allow explicit advances.
- `engine.content.source.read PATH` and `engine.content.source.edit PATH
  SHA256 EDITS`: read a source content file and save JSON pointer edits into
  it under `rusty dev` ([content source](csharp-product-project.md#content-source)).
- `engine.scene.objects`, `engine.scene.object`, `engine.scene.composition`
  and `engine.scene.resources`: readings of the committed scene
  ([scene readouts](#scene-readouts)).
- `engine.inspect.overlay`: what inspection draws ([overlays](#overlays)).
- `engine.inspect.view`: Engine cameras beside the player view
  ([inspection views](#inspection-views)).
- `engine.inspect.target`: the product's named inspection points
  ([inspection targets](#inspection-targets)).

The scene readouts, overlays, inspection views and targets are readings of the
one committed scene and the poses the renderer drew, hosted beside the game:
none of them is a second description of the scene, and none changes what the
product committed.

A playtest harness drives the runtime directly: debug commands through
`/__rusty/product/runtime/debug/execute`, input through a
[harness claim](#harness-input), and world frames through
[tool captures](presentation-capture.md#captures). None of these needs a
page or OS focus. crew-services' `engine` backend works this way, with no
browser.

A page is the lane for the product's DOM UI (focus, text entry, menus, the
pointer-lock shim) and for composite screenshots of the world under the UI.
For it, the browser shell exposes `window.__rustyPlaytest(request)`, which
crew-services' browser backend calls. Each operation runs the same debug
commands; the hook waits until the page has observed that command's outputs,
and, for a command that draws, until the canvas shows the drawn frame.

## Harness input

A harness can drive a product without a page, or while a page or window
watches, by claiming input:
1. `POST /__rusty/product/runtime/control/claim` with
   `{"runtime": <current binding>, "label": "crew-agent-2", "leaseMs": "30000"}`.
   The claim moves the binding (clearing held input) and publishes it with
   `inputClaim`. An attached page stops sending input and shows "Input held
   by crew-agent-2".
2. `POST /__rusty/product/runtime/input` with batches under the claimed
   binding, with sequences from the claim's `nextInputSequence`. Each batch renews the lease. The receipt
   distinguishes queued, admitted and product-observed input as for a page.
3. `POST /__rusty/product/runtime/control/release` with the claimed binding,
   or let the lease lapse on realtime ticks. Either moves the binding again,
   clears what the harness held, and the page takes input back.

While claimed, window focus and page blur change nothing the harness holds,
because the page sends no input: several windows on one machine can each be
driven while another has OS focus.

Claimed input reaches the runtime's input lane directly, so it tests binding
admission and the product's mappings, but not the page's input capture: DOM
focus, text entry, menus and the pointer-lock shim. Test those through a
page. The runtime's current binding for a first claim comes from any runtime
answer that carries one, such as `engine.renderer.presentation`'s `runtime`.

## Product adapter

Register `Rusty.Engine.Debugging.PlaytestDebugModule` in the existing generated
catalog with read-only observation/action delegates and a product look delegate.
`PlaytestAction` describes id, physical keyboard code (or Primary/Secondary/Auxiliary pointer button), live duration in ms, tap
versus hold, current availability/reason and optional equipment. Resolve each
query from current product state. Ordinary input still performs gameplay admission;
a plan's availability does not establish that the subsequent action succeeded.
The product owns observation meaning, action eligibility, timing window, bindings,
look limits and optional `navigation.targets` / `navigation.route` guidance.

The fixed module commands are `playtest.help`, `playtest.observe`,
`playtest.action ID`, and `playtest.look YAW PITCH`. Queries are read-only;
look updates the owning product camera without advancing gameplay.
Doom is the first provider. No ABI or handwritten product bridge is required.

## Tunables panel

A product describes what a developer may tune, the Engine panel renders it,
typed live-debug commands carry the changes, and the
[content source](csharp-product-project.md#content-source) saves them.

The product registers `Rusty.Engine.Debugging.TunablesDebugModule` with a
delegate that builds the current `TunablesPage`, and optionally one that
applies a chooser. A page holds groups of `Tunable`s: a `TunableControl`
(`Slider`, `Toggle` as 0/1, or `Choice` as an option index, with id, label,
range, step and live value) and the owner call that applies a value. A
control may carry a `TunableSource`: a content-root-relative JSON file and a
JSON pointer that hold its saved value (a boolean for a toggle, the option
name or index for a choice). A page may also carry choosers (what is being
tuned), a timeline (keys, duration, the held key, play speeds) with the
product's own `key {key}`, `at {seconds}` and `play {speed}` command
templates, and a notice. Declarations are explicit: no reflection or
attribute scan.

The module's commands answer the `tunables.v1` contract:

- `tunables.describe`: the page as JSON, with the command templates the
  panel uses (`select`, `set`, the content source's `read` and `edit`, and the
  product's own).
- `tunables.select CHOOSER VALUE`: applies a chooser, then describes again.
- `tunables.set ID VALUE`: applies one value, clamped to its range; answers
  the value kept.

`mountTunablesPanel(element, { transport?, describe? })` from
`@rusty-engine/live-debug` mounts the panel. It renders the description and
sends commands; it holds no product state. On each describe it reads every
bound file once through `engine.content.source.read`, resolves the pointers,
and marks a control unsaved when its live value differs from the file's
(floats at half a step). Each bound file shows its unsaved count, **Save**
(one `engine.content.source.edit` of replace operations, with the read's
SHA-256, writing values at the control's step) and **Reset** (each unsaved
control set back to the file's value); each control has its own reset. A
refusal, such as `CONTENT_SOURCE_STALE` after the file changed or
`CONTENT_SOURCE_UNAVAILABLE` outside `rusty dev`, shows with its code, and
outside `rusty dev` the panel still tunes live with no Save. A control whose
change `reshapes` the page describes again after it; groups keep their
folding and the panel its scroll. Style it with the
`--rusty-tunables-*` custom properties or its `rusty-tunables` classes.

Everything the panel does is a live-debug command, so a harness tunes the
same way without a page: `tunables.describe`, `tunables.set`, then
`engine.content.source.read` and `engine.content.source.edit` to save.
`fixtures/csharp-live-tuning` declares a handful of tunables bound to one
file and mounts the panel in the [developer shell](#developer-shell).

## Scene readouts

The `engine.scene.*` commands read the scene as the product committed it: the
presentation world the services hold, not the renderer's tables. Each answer
carries the world `revision` it read.

- `engine.scene.objects [ROOT|*] [DEPTH] [--layer L] [--page N]` lists objects
  depth first in handle order, from `ROOT` or every root (`*`), down `DEPTH`
  levels (all when omitted; `0` lists the roots only), 100 a page. Each has its
  handle, parent and parent joint, kind, layer (its root's), visibility, source
  entity and label, the asset it draws, its local transform, its world
  transform composed through its parents, its bounds, its child count and
  `changedRevision`, the world revision that last created or changed it.
  `total` and `pages` say how many matched.
- `engine.scene.object HANDLE` adds the committed description (the node's
  descriptor as published), material override and instance parameters, the
  pose controls in force on an animated mesh, and the child handles. A mesh's
  or voxel object's `materials` lists each slot as committed (the asset's
  material, or the instance's override over it), and a primitive drawing a
  replaced payload lists its groups' `voxel-material/N` slots with the
  payload's `layerPalette`. Each has its retained material, the standard
  shader features it turns on (`UNLIT`, `WIND`, `ALPHA`, …) and any product
  shader with its path and keywords.
- `engine.scene.composition` answers the cameras, targets, views (with any
  viewport anchor name) and presentations the product committed. The
  `engine.renderer.camera` observer is the renderer's, not the product's, so
  `engine.renderer.presentation` reports it instead.
- `engine.scene.resources` lists the committed descriptors (textures, shaders,
  materials, atlases, meshes, voxel objects) with how many users name each,
  and the admitted resource bodies with their kind and size in bytes.

Limits of a reading:

- A world transform is composed when asked, through the parents' transforms,
  as the presentation world places an entity for audio and particles.
  A node that hangs from a joint (`parentJoint`) leaves out the joint's pose,
  which only the renderer evaluates; read the joint with
  [`ReadJointPose`](skeleton-poses.md#writing-and-reading).
- Bounds are in the node's own space and as the world box around them:
  primitives' unit shapes, a static mesh's payload bounds, and an animated
  mesh's asset bounds at rest, not its current pose.
- Whether an object was drawn or culled is not kept per object.
  `engine.renderer` reports the frame's draws.

`mountSceneTreePanel(element, { transport? })` from `@rusty-engine/live-debug`
mounts a tree over these commands: roots first, a node's children read when it
is expanded, the selected object's facts and committed description, and Refresh
to read them again. It holds no product state.
`fixtures/csharp-skeleton-poses` mounts it.

## Overlays

Overlays are inspection drawing: lines, shapes and labels over the primary
views, drawn only while inspection shows them. While it shows nothing,
nothing is resolved, uploaded or drawn (`FrameStats.overlay_lines` is 0), and
nodes on `RenderLayer.Debug` are hidden too.

A product publishes named sets with `Graphics.PublishOverlaySet(new(name,
items))`. A set stays until replaced; an empty one removes it. Publishing the
same items again costs nothing. Each `OverlayItem` is placed in an
`OverlayPlace`:

- `World`;
- `Model(entity)`: the node the entity's appearance published;
- `AtJoint(entity, joint)`: a rig joint of its animated mesh, as drawn, so an
  item on a hand follows the pose. The joint's position and orientation place
  it, in metres; a rig's joint scale (an armature's unit conversion) does not;
- `Camera`: the primary camera's space, x right, y up, looking down -z, where
  a viewmodel's parts are placed. Items on a viewmodel node or in camera space
  draw with the viewmodel's projection.

The shapes are `Segment`, `Marker` (three crossed lines), `Axes` (red, green
and blue), `WireBox`, `WireSphere`, `Frustum` and `Label`. Each has a linear
colour, and `onTop` draws it over the scene instead of depth-tested against
it. Labels use the Engine's billboard text; a label in viewmodel or camera
space draws on top, placed by each view's own camera as its lines are. An item whose entity or joint draws nothing is not drawn.

The renderer draws its own inspection from what it already evaluated:

- `joints HANDLE`: an animated instance's joint chain, each joint with a
  cross, from the pose it drew;
- `bounds HANDLE`: the world box around a node's parts, the bounds the
  renderer culls with;
- `ik HANDLE`: each two-bone IK chain's target as a cross and the segment
  from the drawn end joint to it, green when reached and red when the chain
  straightened short of it (the solve [`IkChains`](skeleton-poses.md#writing-and-reading)
  reports);
- `camera`: the primary camera's frustum and axes, to see from an
  [observer](#time-drawing-and-camera).

Handles are the ones `engine.scene.objects` lists. `engine.inspect.overlay`
reads what is shown and the product's sets (items, revision, shown);
`engine.inspect.overlay show|hide` with `sets`, `sets in VIEW` (every set,
in that view only), `set NAME`, `debug-layer`, `camera`, `joints H`,
`bounds H` or `ik H` changes one thing. `hide set NAME` also keeps that set
out of `sets` and every `sets in VIEW` (the answer's `hidden`) until it is
shown by name or with `show sets`. A bare `show`
shows every set and the Debug layer; a bare `hide` hides everything. What is
shown is the runtime's, shared by every page, and survives a rebaseline but
not a restage.

Overlays draw in the player view, from the `engine.renderer.camera`
observer and in inspection views. `show set NAME in VIEW` shows a set only in
one view: an inspection view's id, or `player` for the primary views.
Captures leave overlays out unless they ask
([captures](presentation-capture.md#captures)).
`fixtures/csharp-skeleton-poses` publishes a set with an item in each space.

## Inspection views

Inspection views are Engine cameras drawn beside the product's views. They
change nothing the product committed: no camera, pose, stance or aim. The
player view keeps drawing; `engine.scene.composition` does not list them.
`engine.inspect.view` lists them as configured, and
`engine.renderer.presentation` (`submitted.inspectionViews`) the ones the
reported frame drew, with their cameras then.

A view draws in one of two places:

- **An anchored rect.** `create ID anchor NAME` draws the view over the
  player view where the page reports an element anchored under `NAME`
  (`context.viewport.anchor(name, element)` in the product UI). It draws each
  frame the page reports that rect and nowhere else; with no rect it draws
  nothing.
- **Captures only.** `create ID` makes a view that draws only for
  `engine.inspect.view capture` and `frames/capture?view=ID`.

A view draws the world from its own camera, then the viewmodel layer as world
geometry at the primary camera's pose, depth-tested against the world. So a
camera-held assembly stays where the player's camera holds it, however the
inspection camera moves, while the player view keeps drawing it camera-local
with its own viewmodel field of view. Overlays on viewmodel nodes and in
camera space follow it. A view has no per-view resources; closing it
releases nothing but its entry.

`engine.inspect.view`:

- `list` (or nothing): the views, their camera poses and projections, focus
  points and anchors, the selected view, and the player's camera.
- `create ID [anchor NAME]`: a view to the right of the player's camera,
  looking at the space just in front of it.
- `close ID`, `select ID`: remove a view; mark the view that takes pointer
  input.
- `camera ID X Y Z YAW PITCH [FOV]`: an absolute pose (Engine yaw: zero faces
  -Z, positive turns toward +X), and optionally a vertical field of view.
- `orbit ID DYAW DPITCH`, `pan ID DX DY`, `zoom ID FACTOR`: move about the
  view's focus point; pan moves the focus with the camera, zoom scales the
  distance to it.
- `frame ID front|back|left|right|side|top|underside|close SUBJECT`: frame a
  committed object (`SUBJECT` a handle from `engine.scene.objects`) by its
  world bounds, looking along the object's own axes (front from +Z, side and
  right from +X, top from +Y, underside from -Y), or with `close` from where
  the view is, nearer. A viewmodel object's bounds and axes are placed by
  the player's camera, roll included. `SUBJECT` `camera` frames the space just in front of
  the player's camera, and `target NAME` one of the product's
  [inspection targets](#inspection-targets) by its radius and frame. The
  framing leaves 15% around the bounds; pitch stays within 89 degrees of
  level.
- `capture ID PATH [WIDTH HEIGHT] [overlays]`: draw the view into a PNG at
  `PATH` (relative to the host's working directory) and answer its size,
  simulation step, held flag, presentation time, camera and the overlay sets
  drawn. The tool capture route draws views too
  ([captures](presentation-capture.md#captures)).

Hold time with `engine.time.mode manual` to capture the player view and each
view at the same step: every view reads the one committed simulation and
the poses the renderer evaluated.

The scene tree's selected object has **Frame in view**, which frames the
selected view (else the first) on it from the side, and **Bounds**, which
shows or hides its bounds overlay. `fixtures/csharp-skeleton-poses` holds a
model on the viewmodel layer, registers targets and mounts two view panes.

## Inspection targets

A product names the points worth looking at (grips, probes, reach targets,
muzzles) and the Engine frames and marks them; the product never positions an
inspection camera. Register an `InspectionTargetsDebugModule(engine.Graphics,
targets)` with the debug catalog, where `targets` lists `InspectionTarget`s:

```csharp
registrar.Register(new InspectionTargetsDebugModule(engine.Graphics, () =>
[
    new("off-grip", OverlayPlace.Camera, offGrip) { Rotation = item, Radius = 0.1f, Framing = "underside" },
    new("right-hand", OverlayPlace.AtJoint(BodyId, hand), Vector3.Zero) { Radius = 0.15f },
]));
```

A target is placed like an overlay item, so one on a joint or in camera space
follows what it is on. `Rotation` is the frame framings look along, `Radius`
how much around it a framing shows, and `Framing` the one a pane offers
first. Names are single words, as commands name them.
`engine.inspect.target list` (the module's `inspect.targets`) answers the
`inspect.targets.v1` list and publishes the targets as the overlay set
`inspect.targets`: a marker, axes and a label each, drawn while inspection
shows it. `engine.inspect.view frame ID FRAMING target NAME` frames a view on
one, as it is placed now.

## View panes

`mountInspectionView(element, { viewport: context.viewport, id?, subject?,
framings?, transport? })` from `@rusty-engine/live-debug` puts one inspection
view in a product-owned element. The pane anchors its surface through the
product UI's viewport port, so the Engine draws the view there, creates the
view on mount and closes it on dispose. Its bar picks the subject (the player
camera or a product target; a target the product does not offer now falls
back to the camera), frames it with the framing buttons, and downloads a
capture of the view. Its Overlays toggle shows every overlay set in its own
view (`show sets in ID`), reads that state back from the Engine and is undone
when the pane closes, so two panes toggle independently and leave the player
view alone. On the surface, a drag orbits about the focus, a Shift-drag pans
and the wheel zooms; releasing sends what the drag moved since the last send.
Every action is an
`engine.inspect.*` command, so an agent does the same without a pointer.

The surface is interactive UI: while the pointer is on it, the game does not
receive it, and it selects its view on press. Closing the pane gives the
input back. Panes hold no product state; a product mounts them in its own
developer UI, as `fixtures/csharp-skeleton-poses` does under the player view.

What stays approximate: overlays draw what a product publishes and what the
renderer evaluated. A probe marker shows where the product measured, at the
radius it gave; a nearest-triangle "inside" depends on the model's winding.
Neither proves exact contact with a skinned surface, and the Engine makes no
skinned-surface guarantee. A joint's place is the drawn pose, not physics.

## Developer shell

`mountDeveloperShell(element, { panels, presentation?, side?, transport? })`
from `@rusty-engine/live-debug` docks the Engine's developer panels beside
play without pausing it: `{ kind: 'console' }`, `'tunables'`, `'scene'`,
`{ kind: 'view', options }` (an inspection view pane), `'metrics'`, and
`{ kind: 'product', title, mount(element) }` for the product's own DOM.
`presentation: 'dock'` stacks them; `'tabs'` shows one at a time. It starts
closed and holds no panel and sends nothing while closed; `open`, `close` and
`toggle` mount and dispose every panel, and the product binds the key. Its
look follows the `--rusty-shell-*` custom properties, and each panel keeps its
own. It holds no product state.

## Time, drawing and camera

The page's operations are discover, observe, action, look, time, advance,
drawing, frame, camera, targets, route, focus and flush, plus interaction,
grid, probe, clearance and jump-plan when the product registers those modules.
Each maps onto the debug commands below, so a harness without a page runs the
same commands itself; crew-services' engine backend answers the same
operations that way (its `focus` reports that no page is involved).
crew-services adds action orchestration, captures, surveys and recordings on
top.

`focus` is the page's own: it reports canvas focus, pointer lock, the
interaction and cursor modes and whether gameplay holds the pointer.
`{"op":"focus","mode":"capture"}` takes the pointer as a click on the world
would, by the product's cursor mode (a lock, a confined cursor, or focus
alone when unlocked), so an agent gets mouselook without clicking into the
game; `"release"` lets go as Escape does. Each waits up to a second for the
browser and answers with the resulting state, with a note when the browser
did not grant it (it needs a user gesture, which the harness's page
evaluation supplies, and refuses a lock too soon after an Escape). Assisted
capture is not evidence that clicking takes the pointer; test that with a
click.

The world is drawn by the runtime's renderer, so the observer camera, drawing
mode and held time are runtime state that every attached page shares
([architecture](architecture.md#runtime-rendered-output)):

- `drawing` (`engine.renderer.drawing`) is `continuous` (draw every change) or
  `on-demand` (draw only when asked). Frame requests, look and camera changes
  still draw one frame. Resources stay resident.
- `frame` (`engine.renderer.frame`) draws one frame now and answers with
  `engine.renderer.presentation` for it.
- `camera` (`engine.renderer.camera`) reads the observer status or sets an
  observer pose that replaces every primary view's camera. The page also
  accepts a relative move, yaw/pitch, lookAt, or orbit around a target, and
  turns each into an absolute pose. `camera:null` restores the product's
  cameras. The observer never enters player pose, collision or aim. Poses are
  Y-up; positive yaw turns right.

Animation, particles and camera motion run on the Engine's
[presentation time](csharp-lifecycle.md#presentation-time), which advances
only with admitted steps; a running world's frames draw up to a step past it,
so clouds and wind move every frame. Held time freezes them at the committed
time exactly, and `advance` moves them with the steps it admits. Performance metrics still use wall time.
Media and UI playback have their own ownership; these controls concern world
simulation and presentation.

### After `rusty dev` replaces the runtime

A full restage starts a new runtime incarnation, with a new runtime binding and
fresh runtime state. Time is realtime again, the observer camera and drawing
mode are cleared, claims are released and the step count starts over.
- **The attached page recovers by itself.** It re-attaches to the new runtime's
  output stream and takes its binding. The frame view follows the new
  runtime's frames, whose sequence starts again at 1: a frame request after a
  sequence the new runtime has not reached is answered with its latest frame.
- **A harness re-reads.** It re-reads the binding (`discover`, or a fresh
  attachment) and reselects `manual` or `action-driven` time before `advance`.
  Its old binding's claims and input sequence are gone.
- **`503`** means no runtime is serving: during the swap, or after a failed
  restage until the next one succeeds.
- **`422` from `debug/execute`** is a refused debug command (a usage or product
  refusal), not a stale connection.

See crew-services `docs/playtest.md` for CLI examples, evidence handling and
shared-host/reset semantics. Raw keyboard input is unchanged; `assist act` is
the bounded action that pairs physical input with manual advancement.

## Triggered spatial diagnostics

Register `SpatialInspectionDebugModule` with product delegates for the current
session, player pose and controller tuning. `SpatialGridSnapshot.Capture` formats
stacked native `ReadMap` collision queries into an XYZ occupancy volume. Each Y
slice contains Z rows whose characters increase along X. Origin, cell size,
dimensions and source/legend accompany the result. Products choose which
dynamic colliders to supply and document that coverage. Empty collision cells do
not prove a route.

`PlaytestTraversal.Probe` samples native rays at ankle, above-step and head heights,
plus nearby downward floor rays. Products can pair these with their latest
`CharacterStepReceipt` to explain blocked movement and rejected steps. Neither
helper runs continuously or advances simulation.

`PlaytestTraversal.JumpToward` produces a short ordinary input plan from live
character tuning and grounded state. The harness turns toward the target, pulses
jump, holds forward and settles with released controls. Acceleration and collision
can change the landing, so consumers inspect the actual endpoint. It introduces
no second controller, teleport, rewind or successful-arrival promise.

`InteractionDebugModule` includes signed yaw/pitch deltas to each candidate point.
These support look adjustment while retaining reach, visibility and availability
checks. The browser assist surface names this query `interaction`; the native
debug command remains `interaction.inspect`.

## Player-sized clearance

`SpatialClearanceSnapshot.Capture` uses the current body height/radius and the
existing native capsule overlap/cast queries. `SpatialClearanceDebugModule`
exposes it as `spatial.clearance x y z`; the browser assist operation is
`clearance`, with world XYZ target feet. Products supply their
current body height, controller configuration and dynamic colliders.

The result distinguishes current overlap, first contact along a straight sweep,
target overlap and a short downward capsule support probe at the target. It
reports contact normal, source, penetration and initial-contact state. A normal
within the slope limit is a support fact, not a guarantee of a usable landing.
A direct sweep does not run the character's step/jump solver or search a route.
All of these queries are explicit and leave simulation time unchanged.
