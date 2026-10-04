# Playtest inspection

The runtime owns simulation steps. Inspection time modes gate its realtime
scheduler; a manual advance keeps the product's fixed-step cadence and uses the
ordinary C# update/input path. Admitted updates keep `Realtime` mode and the
configured fixed delta. Switching back to realtime clears the wall-time
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

## Time, drawing and camera

The page's operations are discover, observe, action, look, time, advance,
drawing, frame, camera, targets, route, focus and flush, plus interaction,
grid, probe, clearance and jump-plan when the product registers those modules.
Each maps onto the debug commands below, so a harness without a page runs the
same commands itself; crew-services' engine backend answers the same
operations that way (its `focus` reports that no page is involved).
crew-services adds action orchestration, captures, surveys and recordings on
top.

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

Animation, particles and camera motion run on the Engine's presentation time,
which advances only with admitted steps. Held time freezes them, and `advance`
moves them with the steps it admits. Performance metrics still use wall time.
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
dimensions and source/legend accompany the result. The helper permits at most
31 cells per axis and 8192 cells total. Products choose which dynamic colliders
to supply and document that coverage. Empty collision cells do not prove a route.

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
`clearance`, with world XYZ target feet within eight units. Products supply their
current body height, controller configuration and dynamic colliders.

The result distinguishes current overlap, first contact along a straight sweep,
target overlap and a short downward capsule support probe at the target. It
reports contact normal, source, penetration and initial-contact state. A normal
within the slope limit is a support fact, not a guarantee of a usable landing.
A direct sweep does not run the character's step/jump solver or search a route.
All of these queries are explicit and leave simulation time unchanged.
