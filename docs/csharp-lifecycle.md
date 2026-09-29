# C# product lifecycle and services

The product lifecycle and the Engine services a product calls during it, plus value and native-lifetime rules. Entry page: [C# SDK guide](csharp-sdk.md).

## Current lifecycle

The generated `IEngineProduct` contract has lifecycle callbacks for `Start`,
`Update`, `Pause`, `Resume`, `Restart`, `Shutdown`, and `Dispose`. Its optional
`CompleteTimeline` callback receives copied, host-admitted completion data and
lets product code accept or reject the product-owned ticket meaning.

- The constructor receives `ProductCreateContext`, including `IEngineContext`,
  admitted product content, and input configuration.
- `Update(ProductUpdate)` receives Engine-owned update facts and copied input
  events, then returns `ProductUpdateResult` when it needs a supported host
  action.
- `ProductCreateContext.Debugging.Snapshot` retains the latest committed
  Rust-owned lifecycle state and runtime binding, including host-only fault and
  control transitions. Its optional latest-update facts remain the last copied
  update delivery rather than a fabricated host callback.
- The runtime, not the product, drives lifecycle transitions and owns host
  integration. Do not create another central game loop or advance Engine time
  yourself.
- Timeline completion is binding-fenced by the Rust runtime before C# receives
  it. Correlation, outcome, and provenance values are copied into safe managed
  data; a product that does not own timeline tickets may leave the default
  rejecting implementation in place.

A product callback is not a transaction. Every Engine operation takes effect
when it returns, and nothing is rolled back when product code or a later Engine
call fails. A refused operation leaves the state as it was. Validate product
policy before issuing mutations and make retry behavior explicit; do not assume
an exception rewinds an Engine world.

Fresh browser attachment reconstructs presentation from committed Engine
snapshots, and `IEngineProduct` has no attach callback. Publish current
presentation during ordinary product lifecycle and updates. Graphics/voxel handles and publication frontiers survive the
baseline. Playback cursors and controller clip phases resume from Engine-owned
update facts, and ghost plates reconstruct from their capture-time source.
Historical sounds, particle bursts, animation cues, and completion callbacks
are not replayed. Continuous emitters restart their cosmetic simulation.

### Particle bursts

`Presentation.EmitParticles` does not need signal registration, an appearance,
or a retained emitter. `SignalId` is a non-empty product label and may repeat;
each call is its own burst. `LogicalId` is for retained emitters and is ignored
by the one-shot call. A minimal valid cube burst inside an admitted product
callback is:

```csharp
engine.Presentation.EmitParticles(new PresentationParticleDescriptor
{
    SignalId = "blast", // product label; repeated labels are distinct bursts
    Anchor = new() { Kind = PresentationAnchorKind.World, Position = centre },
    Visual = PresentationParticleVisual.Cube,
    Visible = true,
    BurstCount = 8,
    MaxParticles = 8,
    LifetimeMinSeconds = 0.5f,
    LifetimeMaxSeconds = 1,
    SizeCurve = new PresentationParticleScalarKey[] { new(0, 0.2f), new(1, 0.1f) },
    ColorCurve = new PresentationParticleColorKey[]
    {
        new(0, new Color(1, 1, 1, 1)), new(1, new Color(1, 1, 1, 0)),
    },
});
```

The zero-initialized descriptor is incomplete: enum zero is not World or
Billboard, lifetime and capacity are zero, and curves are empty. Set the enums
explicitly. Both curves require 2–8 keys, ages strictly increasing from 0 to 1;
sizes must be finite and nonnegative, color channels in 0–1. Lifetimes must be
ordered within 0.01–60 seconds, velocity bounds ordered and finite, and
acceleration finite. `BurstCount <= MaxParticles <= 1024`, `MaxParticles > 0`,
and `Seed <= 9007199254740991` (53 bits) are required.

For billboard smoke, use an admitted `Graphics.OpenResource` image handle as
`Sprite` and set `SpriteFrameCount = 1`. More frames require a positive
`FlipbookFramesPerSecond` (at most 120); cubes require zero. `HasCollision`
enables the explicit collision material and spawn-relative plane/AABB volumes;
set `Collision.LimitBehavior` and each volume's `Kind`. Collision is cosmetic
and does not mutate Spatial or Dynamics.

An invalid descriptor raises a named `EngineCallException`.
Catching an emission refusal permits the callback to continue and publish its
other output. Letting an exception escape faults the lifecycle (see
[Diagnosing native service refusals](csharp-product-project.md#diagnosing-native-service-refusals)).
Valid bursts return `Admitted`, `Clamped` or `Dropped` under capacity pressure.
The [packaged fixture](../fixtures/csharp-particle-emission/ParticleEmissionChecks.cs)
exercises caught refusals followed by cube, billboard and colliding debris
admission in the same callback.

Current `IEngineContext` properties are named service families generated from
the ABI: dynamics, motion, kinematic, spatial, perception, world origin,
voxel, voxel content and presentation, content, authored content, graphics,
presentation, animation, audio, camera view, random, persistence, content
store, and UI. The exact method set is defined by the current generated
`Rusty.Engine` output and Rust ABI source. Mechanics, resolution, and state
machines are ordinary managed helpers, not native context services. See the
[current capability map](csharp-capabilities.md) and do not assume a Rust API
is callable from C# simply because its crate is public.

### Bounded dynamics ropes

`Dynamics.SetFixedTether` and `SetBodyTether` attach a caller-selected ID to
world/body-local endpoints in one `DynamicsWorld`. Initial attachments outside
the maximum distance reject. Updating the same attachment changes its target;
its effective length approaches that target at the supplied rate. `ReadTether` reports effective/target length, distance/slack, caught/taut
state and a sampled force proxy in N. `RemoveTether` returns a released receipt
and preserves body velocity. A removed body invalidates its attached tethers.

`CreateFixedChain` and `CreateBodyChain` own one anchored end and a series of
sphere beads; the final bead is free. Supply ordinary body properties and an
initial end position. Engine spaces the initial beads along that segment and
creates all bodies and links; a refused chain leaves none behind. `ReadChainPoint` returns the anchor at index
zero followed by bead centers in order. `SetChainLength` distributes total
length and reel rate evenly across links. `RemoveChain` removes all its bodies
and links; destroying a body anchor invalidates and removes the chain. Adjacent
bead contacts are suppressed. Ordinary collision groups select nonadjacent
self-collision; terrain collision uses the same dynamics scene as other bodies.
The segments between beads have no collision geometry.

Defaults are four subdivisions and eight solver iterations per supplied tick;
`ConfigureRopes` selects others (each at least one). `Dynamics.Step` reports
actual rope link/subdivision/iteration work over the requested ticks.
Higher counts cost more work; they do not create a new update clock. Preserve
ordinary body CCD/sleep policies. Fixed endpoints follow `WorldOrigin` rebasing.
Force readouts sample terminal solver-substep impulses, so they are not peak
catch loads or a breakage policy. See the [physics contract](rope-physics.md).

### Composing graphics

Use `context.Graphics` for resources and retained facts; `Appearance` still
names a selected visual resource. `AppearanceFact` carries `ObjectId`,
`HasParentObject`, `ParentObjectId`, local `Transform`, `Appearance`, `Visible`,
and `Layer`. The Engine retains each object's last fact.

- `Graphics.PublishChanges(new(upserts, removals, attachments))` creates or
  updates the listed objects and removes the listed identities. Its cost follows
  the batch, not the world: objects not named are not examined. Each upsert is
  that object's complete fact. An unchanged upsert does nothing, and removing
  an absent object does nothing.
- `Graphics.PublishSnapshot(facts)` makes the facts the complete object set and
  removes every omitted object. It compares each fact with the retained one, so
  it still costs time per object. Use it when a product already rebuilds all
  its facts; publish moving objects with `PublishChanges`.

Either call may list parents and children in any order; the Engine validates
the hierarchy against the state after the whole batch and creates parents
first. A removed object's children must be removed or moved in the same batch.
A refused call changes nothing. Changing an appearance in place, such as its
materials or sprite frame, updates the objects showing it without republishing
them. `EntityGraphicsProjection` also accepts an optional parent `EntityId`.

- Attached equipment: publish the actor and equipment as ordinary facts, with
  the equipment parent naming the actor. Product code selects equipment and
  local placement; Engine owns hierarchy and resource realization.
- Target indicators: compose child sprites/meshes with anchored billboards;
  choose depth-tested, occluded, or always-on-top layers through `Presentation`.
- Procedural effects: combine admitted meshes/materials with lights and
  retained emitters or explicit bursts. `Graphics.CreateMeshResource` also
  admits runtime-generated triangle streams as a disposable Engine resource.

Look math is `Look.Integrate(request)` (and `Reset`, `Rebase`, `Diagnose`) in
the managed toolkit. Replace former `context.Look` calls with these helpers.
Use `Look.IntegrateClamped` for interactive pointer/stick input: it saturates
large finite angular deltas at the configured limit so a quick mouse turn does
not throw out of the product update. `Integrate` retains strict rejection for
commands that must remain within that bound.

Controller sticks arrive as `ControllerAxis` physical input with the normalized
value in `ProductInputEvent.X`. Analog buttons (including standard-gamepad
triggers) arrive independently as `ControllerButtonValue`, with `X` in `[0, 1]`;
`ControllerButton` continues to carry digital press/release edges. Products can
map `controller-button-value:button-7` to an `axis` intent for proportional right
trigger input, just as `controller-axis:axis-0` maps the left stick's X axis.
The runtime retains these scalar values between samples and clears them with
the input lane. Products own dead zones, movement meaning, and stick look speed;
integrate a held stick's angular rate using admitted simulation time, rather
than treating each input sample as a mouse displacement.

### Runtime-generated geometry

Ordinary `MaterialRequest` exposes opaque, mask/cutoff, and blend alpha modes.
Sprite requests accept a `SpriteMaterialDescriptor` for lighting, normal/depth
maps, alpha, and shadow policy. Sprites and atlases retain the sampler selected
when their texture was opened; the short constructors preserve existing
opaque mesh and unlit/blended sprite defaults.

`Graphics.CreateMeshResource(new MeshResourceCreateRequest(positions, normals,
uvs, colors, indices, groups, bindings))` copies ordinary managed arrays into
an immutable retained mesh. Positions/normals are `Vector3`; optional UVs are
`Vector2`; optional vertex colors are linear `Color` RGBA; indices are `uint`.
`MeshGroup` ranges tile the triangle index list, and `MeshMaterialBinding`
selects an existing Engine material for every used slot. Bounds are computed by
Rust. Generated mesh admission has no copied-byte or encoded-byte policy cap.
`GraphicsMeshLimits` describes the managed span count representation; the native
mesh layout stores vertex/index counts as `u32`. Matching streams, finite data,
valid indices and material bindings are checked as part of the final mesh
validation. There is no preliminary serialization solely to measure JSON size.
The current 256-group/binding restriction remains a separate review candidate.

Packed mesh resources use `u32` byte lengths and offsets; they have no 64 MiB
per-resource or 256 MiB retained-set policy cap. Explicit pack sizing remains
a caller choice. Renderer-preload byte and collection quotas are removed; the
Audio service retains its encoded clip budgets ([audio policy](recorded-audio.md)). Texture
dimensions are checked against the active browser GPU before retained PNG
decoding, with no fixed 4,096-pixel or texel-count policy in the model/catalog.
Generated presentation output has no default aggregate
byte/count cap: a large delta is one output batch, without rebuilding the scene.
Allocation and browser/backend capacity still apply. Browser embedders may
select a per-output-batch `maximumOutputBytes` budget.

Create one or more appearances with `Graphics.CreateMeshAppearance(mesh)` and
publish ordinary `AppearanceFact` values. Existing static-mesh material
updates can override instance slots. Geometry is visual-only; C# still selects
any separate spatial mechanism its gameplay needs. To change geometry, admit a
new immutable resource and publish the replacement appearance. Product arrays
may be reused immediately after admission.

`Graphics.PartitionMesh(new MeshPartitionRequest(mesh, origin, cellSize))`
prepares spatial render sections from an existing inline mesh. Origin and
positive cell sizes are in mesh-local units. Whole triangles are assigned by
centroid to deterministic grid bins; attributes, winding and material slots
are preserved exactly, with shared vertices duplicated between sections.
Bounds enclose actual vertices, including triangles crossing a grid boundary.
This changes neither voxel extraction nor collision, and individual sections
are not closed solids.

The returned `MeshPartition` is disposable. `ReadMeshPartition(partition).PartCount`
is the stable slot count. `TakeMeshPartitionPart(new MeshPartitionPartRequest(partition,
index))` transfers each slot once into an ordinary owned `MeshResource`; create
and publish its appearance through the usual APIs. Disposing the partition frees
untaken sections; taken meshes remain independently owned. The source mesh may
be released after preparation or retained as the unchanged collision source.
Partitioning does not perform occlusion culling or promise fewer draw calls.

Remove appearances from the published snapshot before disposing them, then
dispose their mesh resource. Dispose bound materials after their resources and
appearances. The Engine releases unused mesh definitions and GPU geometry;
a browser reconnect reconstructs only current retained geometry. See the
[procedural mesh fixture](../fixtures/csharp-mesh-composition) for a C# shockwave
composed from these primitives.

### Retained camera composition

`CameraView` retains cameras, offscreen targets, and a complete typed
`CameraCompositionRequest`. A composition view selects a live camera, its
normalized viewport, ordering, and either the primary surface or one Engine
target. Presentations copy an offscreen target into a normalized primary
destination, so split-screen and an inset/rear view remain Engine-rendered.
Target revisions and GPU lifetime belong to Rust and the renderer; C# must not
use a target as an arbitrary mesh texture. `SetActiveCamera` remains the
single-primary-view convenience over this same retained composition. Use
`CameraViewports` for ordinary full, split, and inset normalized rectangles.
A composition with any primary view owns the primary surface: the renderer
draws no separate default-camera pass, and area outside every primary view is
left cleared.
`SetBackgroundColor(new(new Color(r, g, b, 1)))` selects an opaque retained
viewport clear color and replaces any selected sky. `SetSkyBackground` replaces
that color with a retained panorama; `ClearSkyBackground` returns to the Engine
default. Products choose the color or resource while the Engine owns renderer
state and realization. `SetSkyBackgroundBlend` blends two retained panoramas from a product-supplied value without rebuilding resources; see [lighting and skies](lighting-and-sky.md) for clock composition and voxel direct-light sampling.

`UpdateCamera` still applies immediately. Opt into render-time sampling with
`UpdateCameraSample(new CameraSampleRequest(camera, descriptor, sampleTimeSeconds,
delaySeconds, CameraInterpolation.Position, cut))`. Use the admitted simulation
facts for a monotonic sample timeline; a useful end-of-batch timestamp is
`(facts.SimulationStep + facts.AdmittedStepCount) * facts.FixedDeltaSeconds`.
`Position` interpolates translation while using the latest published orientation;
`Pose` also interpolates orientation, including explicit-basis roll. `Latest`
returns to immediate presentation. A one-step delay is a starting point, not an
Engine-wide policy. Look remains limited by its publication cadence in position
mode: this API does not predict input or run gameplay in the browser.

Pass `cut: 1` on teleports, origin rebases, and discontinuities; ordinary samples
use `cut: 0`. Camera replacement, browser runtime recovery, timeline regression,
and interpolation-mode/delay changes discard history. Repeated retained snapshots
do not create new samples. Missing samples hold the latest pose without
extrapolation. If receipt time advances more than the source timeline by twice
the configured delay, the recovered sample resets the clock mapping; a paused
source clock cannot permanently disable interpolation. The renderer keeps at
most 64 recent samples per camera; a delay
requiring older history holds the oldest available pose until it can interpolate.
The renderer maps the product timeline to its local clock; its receipt/sample
timestamps are not cross-process latency measurements.

This is presentation only. Gameplay rays and selection continue to use the
product's authoritative camera. A delayed image may therefore differ from a
current gameplay hit, especially while moving close to objects. Products choose
whether this tradeoff is appropriate. Renderer submission diagnostics expose the
actual presented camera separately from `sourceCameras` and camera sample timing;
they establish CPU submission, not GPU completion or streamed-frame correlation.

### Atlas sprite playback

`Graphics.CreateSpritePlayback` retains one admitted sequence for an
atlas-backed sprite. `SelectSpritePlaybackFrame` selects the start of an exact
sequence entry and atomically updates both the playback readout and rendered
sprite frame. It preserves stopped, playing, or paused state; a completed
one-shot becomes paused so it can resume from the selected entry. Selection
does not wrap an out-of-range index, change the current loop cycle, or report
marker crossings because it does not traverse time. A later
`AdvanceSpritePlayback` continues from the selected cursor through the ordinary
Engine-admitted update facts.

### Texture sampling

`Graphics.OpenResource(new RenderResourceRequest(path))` keeps nearest filtering
and clamp wrapping for PNG textures. Ordinary tiled meshes can select sampling
explicitly:

```csharp
var texture = engine.Graphics.OpenResource(new RenderResourceRequest(
    "textures/stone.png", TextureFilter.Nearest, TextureWrap.Repeat));
```

`TextureFilter.Linear` is also available. Sampling applies to PNG resources;
mesh and font resource requests keep their existing behavior. The same PNG can
be selected with different samplers: pixel content remains shared, while each
sampler has its own retained texture identity. Keep sprite/atlas resources
clamped unless their authored usage calls for something else.

### Ghost plates

`IEngineContext.Presentation` provides the retained ghost-plate path. Create a
plate from the stable object ID of a retained Engine `Appearance`, then let the
product choose its placement, capture/framing/lighting settings, plate
mapping, depth/shell settings, and directional profile. The profile supports
1, 4, 8, or 16 captures with hysteresis; sector changes are Engine-owned hard
snaps. `UpdateGhostPlate` changes placement/configuration and
`RecaptureGhostPlate` replaces capture settings. Dispose the returned
`GhostPlatePresentation` before its source Appearance is released.

The Engine retains the cloned capture bank, textures, renderer realization,
and disposal. `ReadGhostPlate` returns copied facts rather than renderer
objects: source presence and match, whether a host observation exists,
fallback/limitation facts, current sector and offset, the effective
capture/configuration, and retained resource counts/timing when the host
provides them. The capture freezes the source Appearance pose, so this is a
bounded presentation mechanism rather than live animation or a second
renderer.

### Microvoxel objects

`IEngineContext.VoxelContent.AdmitMagicaVoxelObject` is the direct small-object
path. It admits bounded MagicaVoxel v150 model bytes with product-selected
identity, source path, cell size, pivot, orientation, and limits. Read the
copied palette/object facts, bind every admitted palette slot to an ordinary
`Appearance` material (roughness 1 is a suitable matte starting point), and
call `ProjectObject` to obtain a retained `VoxelObjectPresentation`. Use the
generated update operation for frame, transform, and visibility changes, then
dispose the presentation and object handles normally.

The default object mesh is a greedy voxel surface with axis-aligned face
normals. This route uses ordinary Engine materials and retained mesh resources;
it does not require voxel-specific shaders, a browser renderer, or TypeScript
game code. It remains bounded by source, dimension, voxel, frame, mesh, and
material limits, and it is not a general scene import path. Unsupported source
or presentation needs are an upstream Engine task and a valid stopping point.

`VoxelScenePresentation` projects the canonical `Spatial` session voxel scene
through the Engine renderer. Bind every currently used scene material slot to
a live `Appearance` material, retain the disposable projection, and call
`RefreshScene` after voxel edits, residency changes, or origin changes. A
refresh after one voxel change visits only the chunks that change named; after
several changes, a replaced scene or an origin rebase it compares each chunk's
mesh hash instead. For a
`GreedyCubes` session, `ProjectSceneDirectional` and
`UpdateSceneDirectional` additionally accept sparse `SpatialFace` overrides;
omitted faces use the required base slot binding. `ReadMaterialMapping` returns
copied effective source-slot/face selections, material provenance values, and
renderer slots. The Engine keeps incremental renderer identity and owns all
generated mesh/frame work; C# receives only copied facts. `Clear` or disposal
stages the matching renderer destroys. Select `VoxelSurfaceMode` in
`SpatialSessionConfig` when creating the session; it chooses only the
Engine-derived mesh posture and is retained through subsequent voxel changes.
Changing the mode of an existing session is not currently a C# API.

### Voxel material collision

Immediately after creating a Spatial session, call
`Voxel.ConfigureMaterialCollision(new(session, declarations))` with
`VoxelMaterialCollision(slot, collidable)` values from the product's material
definitions. Unlisted slots collide, preserving the default occupied-cell
behavior. Duplicate slots return a named operation diagnostic. Configuring again
later replaces the declarations and updates collision for the whole scene.

A false declaration keeps the canonical voxel, material, and visual mesh while
excluding that slot from collision and navigation. Rays, sweeps, and character
steps therefore pass through water and reach solid ground below it. The session
retains the declarations through edits, residency and origin rebases.

Collision has no dependency on Graphics resources or a browser. Authored
`Solid`, `Collidable`, and `Occludes` metadata on a rendered material alone
does not configure a Spatial session: pass the same product-owned
`Collidable` values to Voxel. Occlusion and transparency remain separate
rendering concerns. See `fixtures/csharp-voxel-collision` for a packaged
water-over-floor and multi-cell update-loop exercise.

A multi-cell edit may legitimately fill the space occupied by a character.
The edit and character step are separate operations. If bounded penetration
recovery cannot find a valid contact result, `Spatial.ProposeCharacterStep`
returns an `EngineCallException` carrying the native reason (for example
`unresolved-character-controller-penetration`, including remaining depth).
The product decides whether to prevent such edits, relocate the actor, or
otherwise handle the rejected motion. There is no two-cell transaction limit;
the fixture exercises 64-cell batches and subsequent updates.

### Voxel scene material palettes and atlases

A `GreedyCubes` scene can bind materials from multiple authored voxel atlases,
including distinct atlases over the same texture. Atlas identity, region,
texture and alpha mode belong to each material; there is no scene-wide atlas.
Create each material with `Graphics.CreateAuthoredMaterial` and its selected
texture resource, then bind its source slot through `VoxelScenePresentation`.
An atlas reference must retain the catalog's pinned version/hash. Structural
class does not select a scene atlas or replace canonical voxel state.

Base bindings must cover every currently meshed source slot, with no duplicate
slots. The retained palette may also contain slots not currently meshed, so
adding/removing voxels or streaming chunks does not require trimming the palette.
Face overrides require a corresponding base binding, even when that slot is
currently absent. `RefreshScene` uses the retained palette; use `UpdateScene`
or `UpdateSceneDirectional` when adding a new material binding. `MaterialCount`
counts the retained base palette, including currently unused slots.

There is no three-material limit. Source slots are unsigned 16-bit identities
(0–65,535); base and face bindings share 65,536 renderer slots across retained
voxel presentations. Sixteen base materials in one GreedyCubes scene are
supported. Out-of-range source bindings name the slot, material handle and limit;
renderer-slot exhaustion names the binding and total capacity in the typed
operation diagnostic. Catch `EngineCallException` to keep using the previous
projection. The [sixteen-material fixture](../fixtures/csharp-voxel-capacity)
loads one atlas, renders all sixteen materials, rejects source slot 65,536 and
successfully refreshes afterward. Consumers of older pairs should adopt the
current matched SDK/runtime rather than keeping a three-binding product guard.


Projection, refresh and material-update failures throw `EngineCallException`.
Inspect `Service`, `Operation` and `Diagnostics.Span` for the Engine code and
message (for example, the missing source slots), rather than just the numeric
status. A caught binding rejection leaves the existing projection usable.
The packaged [two-atlas fixture](../fixtures/csharp-voxel-atlases) exercises
solid/decorative and opaque/blend materials, unused palette entries, and caught
rejections followed by successful refreshes.


`Spatial.EvaluateNavigationStep` evaluates one bounded planar-navigation step
against the retained session projection and returns the same typed outcome,
next-waypoint, path-cell, and path facts as `ProposeNavigationStep`. Evaluation
does not update the retained navigation path, revisions, projections, or other
session state, so product code can use its facts before deciding whether to
issue a separate stateful operation. `ProposeNavigationStep` remains the
stateful path proposal and updates the retained path on success or clears it
for its existing failure outcomes.

`Spatial.ReplaceCollisionNavigation` derives and retains a planar projection
from the session's current Engine collision scene. The request supplies a
finite `WorldMin`/`WorldMax` volume plus cell, bounded grid, step, agent
clearance, and maximum-slope policy; it never supplies geometry. The Engine
samples voxel and retained static-mesh collision for support and headroom,
preserves supported elevations, and checks a standing capsule at each cell
center. Directed connections between supported cells use the character
collision capsule casts and step solver: a traversable floor lip can connect
without admitting a thin separating wall or insufficient headroom. Agent radius
and height define capsule clearance; the step limit is cell size times
`MaxStepCells`. Path, weighted-path and navigation-step queries retain these
edge checks alongside product traversal overlays. It considers at most eight
support layers per X/Z cell, so a
deeper layer is intentionally unknown rather than implied walkable. Use the live foot position for `EvaluateNavigationStep` so it
can reconcile to the nearest retained support within one quarter of a navigation cell (capped at 0.1 world units) in
that X/Z cell. This is a
bounded route suggestion only: normal character collision and controls remain
the authority for physical movement. Product door and hazard state belongs in
the existing planar traversal overlay; read-only evaluation honors that overlay
without replacing the retained path diagnostic.

### Collision navigation coordinates

`ReplaceCollisionNavigation` uses a **world-aligned grid with origin (0, 0, 0)**
in the session's current coordinate frame. `WorldMin` and `WorldMax` select
where collision is sampled; they do not translate the grid. `ChunkSize` groups
cells and `GridId` identifies the grid; neither changes its origin.
`RequestNavigationPath`, `RequestWeightedNavigationPath`, traversal cells and
returned path cells use these same signed coordinates.

For cell size `s`, a retained support at world position `(x, supportY, z)` has:

```text
cell.X = floor(x / s)
cell.Y = floor(supportY / s)
cell.Z = floor(z / s)
```

Use mathematical floor, including for negative values. The level is the
**collision surface's support height**, not the occupied floor voxel's index,
agent center, height above `WorldMin`, or a layer ordinal. For example, with
`s = 0.5`, support `(-10.25, -2.0, 17.75)` maps to `(-21, -4, 35)` regardless
of the publication box minimum. A solid voxel at `(-21, -5, 35)` supplies that
top surface. `WalkableCellCount` counts retained supported cells over all sampled
levels. Being inside the box alone does not establish walkability: support,
headroom, slope and capsule clearance still have to pass.

When a caller already knows the sampled support height, it can derive a cell:

```csharp
static PlanarNavCell CellAtSupport(Vector3 support, double cellSize) => new(
    (long)Math.Floor((double)support.X / cellSize),
    (long)Math.Floor((double)support.Y / cellSize),
    (long)Math.Floor((double)support.Z / cellSize));
```

For creature movement from live foot positions, prefer the existing
`Spatial.EvaluateNavigationStep(new NavigationStepRequest(session, fromFeet,
targetFeet, maximumStepDistance, maximumVisitedCells))`. It resolves each
position to the nearest retained support in its world-aligned X/Z column within
`min(s * 0.25, 0.1) + 0.001` world units. This handles the controller's small
standing clearance without searching grid levels. A position with no support
within that tolerance returns `StartNotWalkable` or `GoalNotWalkable`; it does
not snap to a distant floor or a different column. Supply both endpoints in the
same current session frame as the collision publication.

The receipt's `NextPathCell` remains a grid identity; `NextWaypoint` is a
world-space movement proposal bounded by `maximumStepDistance`. Use that
proposal with ordinary character collision. For an intermediate path cell, its
X/Z center is `((X + 0.5) * s, (Z + 0.5) * s)`, but its level only identifies a
height interval: the retained support can be sloped or fractional. The Engine
uses that support height when constructing its proposal. Evaluate is read-only;
`ProposeNavigationStep` also retains the resulting path for indexed inspection.

The [packaged mapping fixture](../fixtures/csharp-navigation-mapping/NavigationMappingChecks.cs)
checks positive and negative coordinates, non-unit cells, unaligned publication
bounds, every reported walkable cell, a multi-cell route, and world-position
steering. It also reproduces `StartNotWalkable` from subtracting the box minimum.
Run it with `scripts/test-csharp-sdk-package.sh --coreclr-smoke`.

Spatial trigger definitions remain registered for the session while their
active state can change. `ReconcileTriggers` and `RestoreTriggers` read the
product's collider rows for that call only: a row whose entity is a registered
trigger is that trigger's world-space AABB, and every other row with enabled
collision is a candidate subject. `SetTriggerActive` deactivation removes
current overlaps and publishes bounded exit facts, while reactivation
publishes no synthetic enter—the next ordinary `ReconcileTriggers` observes
real geometry and produces any new edge. `RestoreTriggers` accepts the complete
active trigger ID set plus current projected colliders and replaces the active
and overlap baseline without producing gameplay facts. Use `ReadTrigger` for
the current active flag, revision, and overlap count, and consume facts only up
to the count returned by the operation receipt. The trigger revision counts
changes to the active and overlap sets; overlap pages use it to fence
continuations. Unknown IDs, duplicate state changes, and duplicate restore IDs
reject without changing the session. Disposing the Spatial session destroys the
definitions, active set, overlaps, and fact history together.

### Generated level artifact admission

Use `Spatial.ReplaceContentArtifact` when an offline generator or importer has
emitted the Engine collision/navigation format through Content. The
[complete example artifact](../fixtures/csharp-spatial-artifact/valid.json) contains
world-space triangle geometry, bounds, and signed multilevel navigation cells.

```csharp
using ContentReference artifact = engine.Content.OpenReference(
    new ContentOpenRequest("spatial-artifact/valid.json"));
try
{
    SpatialContentArtifactReplaceReceipt admitted = engine.Spatial.ReplaceContentArtifact(
        new SpatialContentArtifactReplaceRequest(session, artifact,
            7, 8, 1));
}
catch (EngineCallException error)
{
    // Diagnostics carries the named refusal. The previous world remains usable.
}
```

The Engine resolves the immutable reference, validates schema, finite bounds,
triangle indices and navigation coordinates, then prepares collision and
navigation before publishing either. Success replaces all retained static-mesh
assets/instances and planar navigation, clears the previous navigation path and
traversal overlay, and preserves voxel content, residency and leases. The
returned digest, revisions, counts and projection hashes—and
`ReadContentArtifact`—identify the admitted source. Disposing the borrowed
Content reference afterward does not remove the copied spatial state.

Refusal leaves collision, navigation, artifact identity and residency unchanged.
`EngineCallException` reports service `Spatial`, operation
`ReplaceContentArtifact`, and a named diagnostic such as
`CSHARP_SPATIAL_CONTENT_BOUNDS`, `CSHARP_SPATIAL_CONTENT_COLLISION`,
`CSHARP_SPATIAL_CONTENT_NAVIGATION` or `CSHARP_SPATIAL_CONTENT_SCHEMA`.
A stale reference reports `CSHARP_SPATIAL_CONTENT_REFERENCE`. This guarantee
covers this operation; it does not roll back unrelated calls in a callback.
The diagnostic ABI requires a matching SDK/runtime pair.

Products own generation recipes and artifact semantics: required connected
regions, portal/socket pairing, keys, gates and provenance policy. The Procgen
floor document is a product/generator format and must be converted to this
Engine format before admission. Spatial accepts precomputed navigation facts;
it does not rederive support or enforce that every region connects. For
navigation derived from live collision, use `ReplaceCollisionNavigation`.
Content supplies the artifact byte digest; `Content.ResolveReference` can select
an expected path/digest before spatial admission. Spatial does not interpret a
Procgen payload hash or repeatedly hash retained bytes. These semantic checks
belong in the generator/importer or product admission policy before this call.

The [packaged C# fixture](../fixtures/csharp-spatial-artifact/SpatialArtifactChecks.cs) verifies
collision ray hits, navigation, named extent rejection, unchanged residency and
successful admission after a refusal via `scripts/test-csharp-sdk-package.sh
--coreclr-smoke`.

`CharacterStepRequest.Obstacles` is a borrowed, call-local list of active
product-authored colliders. Give each obstacle its stable entity identity,
current transform, local bounds, collision participation, and motion facts;
the Engine uses them for the one controller proposal and returns ordinary
`CharacterMotion`, `CharacterSupport`, and platform facts. Resubmit the
current support transform and obstacle list on later steps so Engine-owned
support/carry continuation can apply; the session never retains product
entities or collider records. Collision uses the existing translation-offset
AABB posture with unit scale; obstacle rotation participates in platform carry
but does not rotate the collider volume.

For a moving retained static-mesh instance, use the call-local
`CharacterStepRequest.MeshInstances` collection. Each
`CharacterMeshInstance` names the admitted `StaticMeshInstance` by its stable
instance ID, supplies the product entity ID that should receive support facts,
and carries the current linear and angular velocity. The Engine uses the
retained triangle mesh for collision and support, then applies translation and
rotation carry from the admitted instance transform. Do not also submit the
same model as a `CharacterObstacle`: the mesh instance is one collision and
support authority. Update its retained pose through `ApplyCollisionResidency`
before the step and resubmit its mesh admission each step. The separate
`CharacterSupport` value may be absent for admitted mesh support; the Engine
reads its pose from residency. An unadmitted mesh remains collision-only.
Product persistence keeps the instance/entity IDs
and pose as ordinary values; no native handle is part of the saved state.

PrimitiveGeometry.Line creates an ordinary retained line from local `(0, 0, 0)`
to `(0, 1, 0)`. Set its appearance translation to the first endpoint, rotate
local +Y toward the second, and scale Y by their distance. Publish it through
the ordinary appearance snapshot; Engine owns line realization and cleanup.

### Character tethers

Set `CharacterStepRequest.Tether` with `CharacterTetherRequest.AtFixedAnchor`
or `AtDynamicAnchor`; the default request remains untethered. Use a stable
nonzero attachment ID and a character-local point. Maximum length admits the
initial attachment; target length changes at the authored reel speed. Resubmit
the request each step, or omit it to release while retaining accepted momentum.
`CharacterMotion` carries attachment and effective-length continuation alongside
the existing controlled/external velocity, with no separate swing state.

For a dynamic anchor, call `Dynamics.ObserveAnchor` with its body and local
point before each character step; a disabled body returns an invalid
observation and the character receipt reports invalidation. Engine resolves point velocity, center of mass,
and impulse response including inertia and locked axes. Product code need not
calculate these. The character uses effective mass and this response to share
the velocity correction, capped by `MaximumDynamicImpulse` on both sides.

`CharacterStepReceipt.Tether` reports endpoints (in character-to-anchor order),
effective maximum length, separation, taut/caught/released/invalidated state,
radial and tangential velocity, swept correction, saturation and unresolved
separation. A dynamic receipt also contains `Reaction`. Apply the chosen
reactions explicitly through `Dynamics.StepWithReactions` together with ordinary
actions; this performs one normal Dynamics step, applying each reaction as an
impulse at its observed point. A reaction carries no revision, so applying it
twice applies it twice. Observe anchors, propose character steps, then apply
the reactions. There is no hidden Dynamics step inside the character controller.

Terrain can prevent a length correction, and the impulse cap can leave the rope
extended; inspect `Unresolved` rather than assuming an exact rigid constraint.
Reeling is caller-authorized work, not a promise of energy conservation. Use the
receipt endpoints for ordinary debug-line presentation. The public-facade
exercise is in `fixtures/csharp-nativeaot-trial/Product.cs`.

To save an admitted character continuation, call
`CaptureCharacterContinuation` with the latest `CharacterStepReceipt.Generation`
and persist the copied `CharacterContinuationCheckpoint` beside the
product-owned pose and look. After recreating a compatible `SpatialSession`
and its canonical content, call `RestoreCharacterContinuation`; use its
returned `Motion` and the checkpoint's `Config` for the next
`ProposeCharacterStep`, while supplying current product-authored support and
obstacle facts as usual. The Engine rejects stale source generations, invalid
motion, changed session configuration, and changed canonical content before
returning a continuation. A checkpoint is a plain value, not a session lease;
it cannot restore into a disposed or already-used target session. Its source
session identity and generation are copied diagnostic provenance, not a native
handle that remains resolvable after save/load; target compatibility comes from
the typed configuration, motion, session, and canonical-content checks.

## Values, leases, and native lifetime

The public C# layer turns direct service calls into typed requests, receipts,
values, and disposable handles. Follow the type's ownership model:

- Use returned value records directly or copy their data when you need to keep
  it.
- Dispose values that represent an Engine lease, session, snapshot, resource,
  or handle when their scope ends. `using` is the usual product-side shape.
- Do not retain borrowed spans, native pointers, or callback-backed data beyond
  their stated call/lease lifetime.
- Do not add unsafe code, handwritten P/Invoke, ABI structs, or
  `UnmanagedCallersOnly` exports to normal product code. The generator owns
  those details.

The product boundary is trusted, but these memory and lifetime rules are real
correctness requirements rather than policy ceremony.
