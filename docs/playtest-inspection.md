# Forward-only playtest inspection

The existing native lifecycle owns simulation steps. Inspection mode gates its
realtime scheduler; manual admission keeps the product's fixed-step cadence and
uses the ordinary C# update/input path. Switching back to realtime clears the
wall-time baseline. Holding does not stop input admission or renderer inspection.

Native live-debug commands (require the ordinary live-debug opt-in):

- `engine.time`: effective mode, step count, cadence and held status.
- `engine.time.mode realtime|manual|action-driven`.
- `engine.time.advance MS`: positive duration up to 2000 ms, rounded up to fixed
  steps, returning actual advancement. Both held modes allow explicit advances.

The Engine browser host exposes `window.__rustyPlaytest(request)` for the local
crew-services driver. It waits for the existing output boundary before capturing
new projection state. This uses the existing ordered transport and renderer;
there is no replay or second simulation scheduler. Connection baselines carry
an observed output boundary so a held host can accept feedback before another tick.

## Product adapter

Register `Rusty.Engine.Debugging.PlaytestDebugModule` in the existing generated
catalog with read-only observation/action delegates and a product look delegate.
`PlaytestAction` describes id, physical keyboard code, live duration in ms, tap
versus hold, current availability/reason and optional equipment. Resolve each
query from current product state. Ordinary input still performs gameplay admission;
a plan's availability does not establish that the subsequent action succeeded.
The product owns observation meaning, action eligibility, timing window, bindings,
look limits and optional `navigation.targets` / `navigation.route` guidance.

The fixed module commands are `playtest.help`, `playtest.observe`,
`playtest.action ID`, and `playtest.look YAW PITCH`. Queries are read-only;
look updates the owning product camera without advancing gameplay.
Doom is the first provider. No ABI or handwritten product bridge is required.

## Browser time, drawing and camera

Operations: discover, observe, action, look, time, advance, drawing, frame,
camera, targets, route, focus and flush. crew-services adds ordinary keyboard
action orchestration, captures, surveys and recordings on top.

`drawing` accepts continuous or on-demand. Continuous explicitly requests actual
full-rate draws, even with held simulation. On-demand preserves the existing
cadence for input sampling and world presentation updates, but skips automatic
draw calls. Frame requests and camera inspection can still draw. Resources remain
resident; this is not a GPU-memory eviction feature.

Held simulation anchors presentation time to native step progress. Animation and
particle deltas stop while held and advance with requested simulation time.
Camera interpolation uses the current pose while held, so inspection never waits
for a paused interpolation clock. Performance metrics still use wall time.
Media/UI playback has its existing separate ownership; these controls concern
world simulation and retained world presentation.

`camera` reads pose/observer status, accepts an absolute pose, relative move,
yaw/pitch, lookAt, or orbit around a target. `camera:null` restores the current
product camera. The renderer override never enters player pose, collision or aim.
Camera coordinates use the renderer's Y-up convention; positive yaw turns right.

See crew-services `docs/playtest.md` for CLI examples, evidence handling and
shared-host/reset semantics. Raw keyboard input is unchanged; `assist act` is the
bounded action that pairs physical input with manual advancement automatically.

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
