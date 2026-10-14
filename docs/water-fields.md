# Water fields

`engine.WaterField` runs a region of moving water over ground the product
describes: a river that rises in a storm and spills onto its floodplain, a
hollow that fills and soaks away, a dam that backs a river up. The product
gives a region's bed heights, what lies beyond its edges and where water is
added or drained. The Engine advances the water on its own worker, publishes
complete snapshots, answers samples by position, and can draw the surface.

```csharp
WaterField field = engine.WaterField.Create(
    new WaterFieldCreateRequest(width, length, cellSize, bedHeights, corner));
engine.WaterField.SetBoundary(WaterFieldBoundaryRequest.Inflow(field, WaterFieldSide.XMin, 6, first: 18, count: 4));
engine.WaterField.SetBoundary(WaterFieldBoundaryRequest.Open(field, WaterFieldSide.XMax));
engine.WaterField.Present(field, waterMaterial);
// Any update:
WaterFieldSample here = engine.WaterField.Sample(field, swimmerPosition);
```

`fixtures/csharp-water-field` floods a river valley with a hollow beside the
channel, dams it, and lets a swimmer swim in the flood.

## What the water is

A field is a grid of square cells with one bed height and one water depth
each: column water, one surface over any point. It covers rivers, lakes,
floods, currents and drains. It does not model stacked water (a river over a
cave), pouring, spray or splashing.

Water moves between neighbouring cells by the difference in their surfaces,
slowed by bed roughness (Manning's `n`, `WaterFieldPolicy.Roughness`, 0.03 by
default for an earth channel). A face carries only the water standing above
the higher of its two beds, so a hollow spills only over its rim, and depth
never goes negative. Steady flow down a slope settles at Manning's depth for
that slope and roughness. A wave on dry ground travels slower than a real
dam-break front: about a third of the frictionless speed, at any cell size.

The solver reads field units as metres and seconds, with gravity 9.81.

## Ownership

The Engine owns the regions, their worker, the snapshots, sampling and the
drawn surface. The product owns which regions exist and where, their bed
heights, what happens at their edges (a stage or discharge from its own river
network), rain and drains, and what the water means. The product's own water
owner composes a field's answers with any other water it has (a sea, static
lakes), so the rest of the product never learns which answered.

## Regions

`Create` takes the cell counts along x and z, the cell size, a bed height per
cell (x fastest, from the field's corner), optional starting depths, where the
corner stands, and a `WaterFieldPolicy`. Bed heights, stages and bed edits are
in the field's own frame, from its corner. With a `Session`, the corner is the
world point `Anchor` resolved into that spatial session's frame again after
every world-origin rebase, as a heightfield's is. Disposing the `WaterField`
retires it.

- **Edges.** Every edge starts as a wall. `SetBoundary` sets a run of edge
  cells to a `Stage` (a reservoir held at a surface height; water flows in or
  out), an `Inflow` (a discharge in cubic metres a second, spread over the
  run), `Open` (the ground beyond continues the slope of the last two cells,
  so water leaves at the rate that slope drives) or back to `Wall`.
- **Sources.** `SetSources` replaces every cell's rate in metres of depth a
  second: one uniform rate (rain), then per-cell rates (a spring, a drain, a
  soak). A negative rate never takes more water than is there.
- **Bed edits.** `EditBed` gives cells new bed heights (a dam, a breach, a
  dug channel) and bumps the field's incarnation. Water on a raised cell is
  lifted with it and runs off.
- **Policy.** `SetPolicy` changes the Courant number, the longest step, the
  threads one step may use, the backlog and the roughness. Zero selects each
  default.

## Time

A field advances by the simulated time the lifecycle admitted, never by wall
time: as each product call finishes, every field is handed that call's
admitted steps (`FixedDeltaSeconds × AdmittedStepCount`). It pauses with a
[gameplay hold](csharp-lifecycle.md#gameplay-time), slows with slow motion,
and catches up with a bounded advance. A call with no update (create, debug
commands, lifecycle calls) admits nothing.

Changes a call makes (sources, edges, bed edits, policy, drawing) take effect
from the next admitted time, and the worker keeps them in that order: each
field's pending work is a queue of changes, each followed by the admitted time
that runs under it, so rain set for one update and stopped the next falls for
exactly that update however late the worker gets to it. The worker steps
through the admitted time at the stable step its Courant number allows and
publishes a snapshot at the admitted boundary it reaches. The next call reads
the latest snapshot without waiting for the worker. A snapshot finished before
a bed edit reached the worker is not published to the product; samples come
from the previous one until the worker publishes one with the edit.

A field that costs more than it advances falls behind, by a bounded amount.
The policy's backlog bounds the work a field may owe its worker: owed time ×
what a simulated second last cost, in worker seconds (1 by default).

- Admitted time past it is dropped from the oldest waiting intervals, whose
  changes still apply, and reported in `DroppedSeconds`; a batch the worker
  runs also stops once it has spent the backlog in wall time.
- At most 16 intervals wait per field: past that the oldest two merge into one
  span of time, the later one's changes applying at its end, counted in
  `CoalescedChanges`. Pending memory stays bounded even before the first
  advance has measured a cost.
- A cheap field fast-forwarded far ahead of the wall clock drops nothing.

When a field reports a load above 1, drops time or coalesces changes, give it
fewer cells or a larger cell size.

## Samples and reads

`Sample` takes scene points (x and z) and returns, from the latest snapshot,
whether each is inside the field and wet, the water surface and bed height in
the scene, the depth, the depth-averaged velocity, and the snapshot's time and
age (admitted time not yet in it). A point between cell centres takes the bed
of the four nearest and the surface of the wet ones among them, so the surface
stays level up to a shore. `ReadCells` returns every cell's depth, surface (in
the field's frame) and velocity. `Read` returns the diagnostics:

| Field | Meaning |
| --- | --- |
| `Origin` | Where the corner stands in the scene now. |
| `Incarnation`, `SnapshotIncarnation` | The field's bed edits, and those the snapshot carries. |
| `Time`, `AdmittedTime`, `Age` | The snapshot's simulated time, the admitted time handed to the field, the gap. |
| `DroppedSeconds` | Admitted time dropped past the backlog. |
| `PendingSeconds`, `CoalescedChanges` | Admitted time waiting for the worker, queued or being run (neither in the snapshot nor dropped, so `Age` is `DroppedSeconds` plus `PendingSeconds`), and changes applied later than their boundary because too many intervals waited. |
| `Volume`, `BoundaryInflow`, `BoundaryOutflow`, `SourceVolume`, `SinkVolume` | Water in the field, and in total through its edges, sources and sinks. |
| `WetCells`, `DrawnCells` | Cells holding water, and those the drawn surface shows. |
| `Steps`, `StepMicroseconds`, `Load` | The worker's last advance: steps, wall time, worker seconds per simulated second. |

Samples and cell reads are borrowed until the next WaterField call; the
generated SDK copies them.

### Swimming

Character swimming takes a water box per step
([world streaming](world-streaming-contract.md)). A product's water owner
builds it from a sample: the ground to the surface around the character.

```csharp
WaterFieldSample water = engine.WaterField.Sample(field, position);
var movement = water.Wet
    ? new CharacterMovementRequest(CharacterMovementMode.Swimming, vertical, speed, acceleration, drag,
        new(position.X - 1, water.Bed, position.Z - 1), new(position.X + 1, water.Surface, position.Z + 1),
        gravityScale, buoyancy, 0)
    : default;
```

Pass `water.Velocity` as the step's external velocity for the current to
carry the character.

## Drawn surface

`Present(field, material, layer, drawnDepth)` draws the field's wet cells as
one mesh with the product's material, redrawn from each new snapshot. Give it
a blended material with water settings
([water](lighting-and-sky.md#water)) for depth tint and shore foam. The
surface covers the cells deeper than `drawnDepth` (0.01 by default), each
corner at the mean surface of the wet cells around it, and is trimmed to the
bed: each corner's bed is the mean of the cells around it, as ground
interpolated between cell centres runs, and where that bed rises through the
water inside a wet cell the surface ends there. Otherwise it ends at the wet
cells' edge, so the drawn water never reaches a cell the samples call dry.
The material is held while
the field draws with it. A zero material stops drawing. A product may instead
read cells and draw its own surface.

## Limits

- One surface per point; no stacked water, falling water or spray.
- The worker is one thread for all fields, stepping each field in turn;
  `Threads` splits one field's step across threads by rows, worthwhile above
  about 128 × 128 cells.
- A snapshot copies depth and velocity (12 bytes a cell); a drawn field also
  builds its mesh on the worker. A field holds about 36 bytes a cell on the
  worker.
- The GPU does not step fields.
- Nothing is saved: a product that must keep a field captures its own bed
  edits and forcing and replays them.
- Dynamics bodies float on a field through `Dynamics.SetWater`
  ([floating bodies](rope-physics.md#floating-bodies)); the field does not
  feel them.
