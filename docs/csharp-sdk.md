# C# SDK guide

This guide describes the C# surface that exists today. It distinguishes that
surface from recommended product organization so an agent does not mistake a
proposal for an API.

## Build a product through the packaged surface

`Rusty.Engine` is one immutable NuGet package containing the public C# service
surface, managed helpers, generated contracts, and the product generator. An
ordinary Product repository configures its package feed and references only the
package:

```xml
<ItemGroup>
  <PackageReference Include="Rusty.Engine" Version="0.1.0-dev.EXACT" />
</ItemGroup>
```

The ordinary product project also declares the concrete product and its bundle
facts. A realtime product has a shape like:

```xml
<PropertyGroup>
  <RustyEngineProductEntryType>Example.Game.ExampleProduct</RustyEngineProductEntryType>
  <RustyEngineProductId>example.game</RustyEngineProductId>
  <RustyEngineProductTitle>Example Game</RustyEngineProductTitle>
  <RustyEngineProductUiRoot>$(MSBuildProjectDirectory)/../../ui</RustyEngineProductUiRoot>
  <RustyEngineProductContentRoot>$(MSBuildProjectDirectory)/../../content</RustyEngineProductContentRoot>
  <RustyEngineProductLifecycleMode>realtime</RustyEngineProductLifecycleMode>
  <RustyEngineProductFixedStepHz>60</RustyEngineProductFixedStepHz>
  <RustyEngineProductFixedStepMaxCatchUpSteps>4</RustyEngineProductFixedStepMaxCatchUpSteps>
</PropertyGroup>
```

Input intents/mappings and optional UI-projection identity are declared with
the corresponding `RustyEngineProduct*` MSBuild items/properties. The SDK owns
the generated composition below `obj`; a Product must not check in a
`NativeProduct` bridge, generated bindings, exports, or service-table code.

Product code implements `IEngineProduct`, accepts `ProductCreateContext`, and
keeps `IEngineContext` or the named services it needs. Exactly one concrete
`RustyEngineProductEntryType` is declared. The generator supplies both CoreCLR
and NativeAOT bind implementations without assembly scanning or product-side
registration infrastructure.

## Run and package

For a clean downstream CI or developer setup, begin with one verified exact
[SDK/runtime distribution pair](csharp-distribution.md). It supplies the local
NuGet feed and the runtime pack together; ordinary consumption does not need
an Engine checkout, Cargo, binding generation, or copied Engine browser files.

The normal development command uses the `rusty` binary from the exact matching
runtime pack:

```bash
/path/to/runtime-pack/bin/rusty dev \
  --project /path/to/Example.Game.csproj \
  --runtime /path/to/runtime-pack
```

It builds the ordinary project, asks the SDK to atomically stage a loose
Product directory, launches the packaged host through CoreCLR, and restarts it
when declared C#, UI, or content inputs change. `--bind-host`, `--port`, and
`--live-debug` override the corresponding staging properties for a development
session. Use `--debugger` for managed breakpoint sessions; see
[CoreCLR diagnostics](coreclr-diagnostics.md) for worker discovery, profiling,
and the opt-in deadline behavior.

The Product directory has `product.json`, managed output under `coreclr/`, and
Product-owned `ui/` and `content/`. Engine JavaScript and host binaries stay in
the runtime pack. Product UI is DOM UI and accessibility only; the Engine
renderer remains the owner of non-UI presentation.

The package and runtime pack carry exact generated ABI identities. A mismatch
is rejected before product construction. Use a package and runtime pack built
from the same Engine release; do not add version negotiation, copy a host into
the Product, or repair the mismatch with handwritten interop.

NativeAOT is an explicit fidelity/release check, not the edit-run loop:

```bash
dotnet msbuild /path/to/Example.Game.csproj -t:VerifyRustyEngineAot
```

Engine contributors may run `rusty dev --engine-source
/absolute/rusty-engine`. That explicit option selects the source checkout's
runtime pack and supplies `RustyEngineUseSourceDevelopment` plus the absolute
`RustyEngineSourceDevelopmentPath` to MSBuild. The product project must
conditionally exclude the package's compile/runtime assets when that flag is
true, as the SDK directs. Never make this override, an adjacent checkout, or
downstream binding generation the normal product setup.

The fixtures in this repository remain broad provider proof scaffolding. They
are useful when changing the ABI/generator/runtime, but they are not a template
for a downstream repository's launch topology.

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

A product callback is not a transaction over every Engine service. Each
generated service operation preserves its own validation and failure
atomicity, while only service families that explicitly stage call output are
committed or discarded with the outer callback. Mutable `Spatial` and `Voxel`
operations commit immediately: if one succeeds and product code or a later
Engine call fails, the earlier mutation remains authoritative. Validate
product policy before issuing mutations, retain returned revisions/receipts,
and make retry behavior explicit. Use a named prepared/commit API when a
multi-owner change genuinely requires coordination; do not assume an exception
rewinds an Engine world.

Fresh browser attachment reconstructs presentation from committed Engine
snapshots; the host no longer invokes `IEngineProduct.Attach` to rebuild a
renderer. Publish current presentation during ordinary product lifecycle and
updates. The generated `Attach` member remains as an optional default method for
source continuity, but placing required initialization only there has no effect on a
fresh attachment. Graphics/voxel handles and publication frontiers survive the
baseline. Playback cursors and controller clip phases resume from Engine-owned
update facts, and ghost plates reconstruct from their capture-time source.
Historical sounds, particle bursts, animation cues, and completion callbacks
are not replayed. Continuous emitters restart their cosmetic simulation.

Current `IEngineContext` properties are named service families generated from
the ABI: dynamics, motion, kinematic, spatial, perception, world origin,
voxel, voxel content and presentation, content, authored content, graphics,
presentation, animation, audio, camera view, random, persistence, content
store, and UI. The exact method set is defined by the current generated
`Rusty.Engine` output and Rust ABI source. Mechanics, resolution, and state
machines are ordinary managed helpers, not native context services. See the
[current capability map](csharp-capabilities.md) and do not assume a Rust API
is callable from C# simply because its crate is public.

### Composing graphics

Use `context.Graphics` for resources and retained facts; `Appearance` still
names a selected visual resource. `AppearanceFact` carries `ObjectId`,
`HasParentObject`, `ParentObjectId`, local `Transform`, `Appearance`, `Visible`,
and `Layer`. A complete snapshot may contain parents and children in either
input order; the Engine validates the hierarchy and publishes parents first.
`AppearanceEntityWorld` also accepts an optional parent `EntityId`.

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
a caller choice. Texture/audio byte and collection quotas are removed; texture
dimensions are checked against the active browser GPU before retained PNG
decoding, with no fixed 4,096-pixel or texel-count policy in the model/catalog.
Generated presentation output has no default aggregate
byte/count cap: the host fragments large deltas without rebuilding the scene.
Worker messages retain their actual `u32` byte-length representation constraint;
allocation and browser/backend capacity still apply. Browser embedders may
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
`RefreshScene` after voxel edits, residency changes, or origin changes. For a
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

`Spatial.EvaluateNavigationStep` evaluates one bounded planar-navigation step
against the retained session projection and returns the same typed outcome,
next-waypoint, path-cell, and path facts as `ProposeNavigationStep`. Evaluation
does not update the retained navigation path, revisions, projections, or other
session state, so product code can use its facts before deciding whether to
issue a separate stateful operation. `ProposeNavigationStep` remains the
stateful path proposal and updates the retained path on success or clears it
for its existing failure outcomes.

Spatial trigger definitions remain registered for the session while their
active state can change. `SetTriggerActive` is revision-guarded: deactivation
removes current overlaps and publishes bounded exit facts, while reactivation
publishes no synthetic enter—the next ordinary `ReconcileTriggers` observes
real geometry and produces any new edge. `RestoreTriggers` accepts the complete
active trigger ID set plus current projected colliders and replaces the active
and overlap baseline without producing gameplay facts. Use `ReadTrigger` for
the current active flag, revision, and overlap count, and consume facts only up
to the count returned by the operation receipt. Unknown IDs, duplicate state
changes, duplicate restore IDs, and stale revisions reject without changing
the session. Disposing the Spatial session destroys the definitions, active
set, overlaps, and fact history together.

Use `Spatial.ReplaceContentArtifact` when offline conversion has already
published the canonical collision/navigation JSON through Engine Content. The
Engine resolves the retained `ContentReference`, validates and copies its
bounded geometry and signed multilevel navigation facts, and replaces both
projections as one operation. The returned digest, revisions, counts, and
projection hashes—and `ReadContentArtifact`—identify the admitted source.
Products still choose the content and navigation grid policy; they do not read
the bytes, rebuild raw array requests, or infer collision from a visual mesh.

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

## Optional managed helper packages

These packages are current reusable helpers, not required product framework
pieces:

| Package | Current role |
| --- | --- |
| [`Rusty.Engine.Application`](../csharp/Rusty.Engine/Application) | An optional Engine-context update pipeline and deterministic scheduler helper, compiled into `Rusty.Engine`. Its `SimulationScheduler` can resume on the next admitted step, wait fixed admitted steps, or wait for a caller-owned completion condition without creating a second clock. |
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | Product component keys, managed entity worlds, snapshots, batches, and managed adapters around Engine mechanisms, compiled into `Rusty.Engine`. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Product-state codecs, stores, restoration plans, and entity-world persistence helpers, compiled into `Rusty.Engine`. |
| [`Rusty.Engine.Resolution`](../csharp/Rusty.Engine/Resolution) | Structural resolution sessions and typed product transaction coordination inside the default `Rusty.Engine` assembly. |

Use a helper when it fits the product's real domain. A product may compose its
own ordinary C# architecture instead. None of these packages implies a hidden
`ProductApplication`, `ProductBuilder`, `IProductModule`, analyzer suite,
typed-content framework, or projection framework: those names are not current
SDK APIs.

## Recommended product architecture, not a framework contract

[C# product style](csharp-product-style.md) recommends organizing product code
by domain modules, keeping state ownership explicit, and using thin
Read/Decide/Apply/Publish coordinators. These are conventions a product can
adopt directly; they do not require registration APIs or runtime discovery.

For gameplay that needs Engine infrastructure, combine the ordinary managed
helpers with named generated service calls rather than reimplementing native
mechanisms downstream. Product rules, state transitions, and orchestration are
ordinary C# concerns.

## Missing capability workflow

If the needed behavior cannot be expressed through the generated API:

1. identify the missing Engine mechanism and the lifecycle point it needs;
2. record the concrete product call shape or fact the mechanism should admit;
3. file or link the narrow Engine task when authorized; and
4. stop the downstream substitution work.

Do not bypass the gap with a browser renderer, TypeScript gameplay path,
handwritten interop, JSON bridge, or a parallel Rust implementation in the
product repository. The inability to proceed is useful evidence for the
upstream capability work.

## Runtime implicit surfaces

`engine.ImplicitSurfaces` constructs general-purpose scalar fields and generates
ordinary retained `MeshResource` objects. This is independent of the existing
voxel residency service and its cubic surface modes. C# owns the shape recipe,
material selection, and regeneration intent; Rust owns evaluation, dual
contouring, mesh attributes, and renderer admission. No retro style is built
into this service.

Create an `ImplicitField`, add boxes, spheres, ellipsoids, capsules or planes,
and compose their returned `ImplicitNode` values with union, intersection,
difference, smooth union, offset and affine TRS placement. Nodes belong to that
field: their opaque tokens are valid only with the field that produced them, and
discarded or disposed-field tokens are rejected. Values are negative inside; these constructive fields preserve a zero
surface but are not necessarily Euclidean distances. Smooth-union radii and
level-set offsets are in field-value units, especially after nonuniform scale.

`DisplaceWaves(ImplicitWaveRequest)` adds smooth seeded spectral noise to a
source field. `Frequency` selects cycles per coordinate unit on each axis;
`Amplitude` bounds the absolute change in field value. `Octaves` (1–8),
`Lacunarity` (at least 1), and `Gain` (0–1) control the normalized multiscale
sum. The same seed and parameters reproduce the field. This is a finite sum
of independently oriented waves, not lattice Perlin noise or simulated erosion.
Amplitude is not a world-space displacement guarantee. The operation preserves
neither connectivity nor a bounding shell; compose protected volumes afterward
and select extraction spacing appropriate to the finest wavelength. The scoped
`ImplicitRecipe.DisplaceWaves` helper uses this same Engine operation.

`Generate(ImplicitGenerateRequest)` takes an enclosure, sample spacing, crease
angle, UV scale, default material, and optional ordered material regions:

- The enclosure expands about its center into a cube with its longest side,
  preserving uniform world-space samples. To clip to a rectangular volume,
  explicitly intersect a box. Domain boundaries are not automatic caps.
- `MaxExtractionVertices` and `MaxExtractionTriangles` select raw extraction
  output budgets. Zero retains the default 262,144 each; positive values select
  the caller's budget. These are not peak-memory limits or limits on subsequent
  attribute/material splitting. Material-refinement budgets remain separate.
- Cell size is a maximum leaf sample spacing, not a minimum-feature guarantee.
  Thin features can disappear. Keep enough empty margin around closed shapes.
- `AddFrustum` authors a capped circular taper between distinct `Start` and `End`
  points. Non-negative `StartRadius`/`EndRadius` select the endpoint sizes; at
  least one must be positive. Equal radii give a cylinder, one zero radius a
  cone. Caps are perpendicular to the axis. Its negative-inside field preserves
  the zero surface but is not generally Euclidean signed distance, so offsets
  and blends retain the ordinary field-value interpretation.
- Zero crease angle gives flat facets; larger angles admit incident faces into
  area-weighted normals. Major-axis planar UV charts use world coordinates;
  UV scale is repeats per world unit, independent of extraction density.
- The short request constructor keeps `ImplicitMaterialBoundaryMode.Centroid`:
  each triangle uses the first region containing its centroid, or the default
  material. Select `MaterialBoundaryMode: ImplicitMaterialBoundaryMode.Interpolated`
  in the full request to split triangles at the zero contour of linearly
  interpolated vertex field samples. This gives exact cuts for affine fields
  such as planes, independently of triangle direction. Curved fields are
  polygonal approximations; regions hidden between vertices can be missed.
  Set `MaterialSampleSpacing` to a positive world-space edge length to subdivide
  the attributed surface before sampling material fields, independently of DC
  simplification. Zero retains the existing behavior. This option requires
  `Interpolated`; it does not change geometry extraction or recover missing
  grooves. Shared edges subdivide consistently and new normals/UVs interpolate
  the original attributes. Choose spacing below the narrowest desired motif;
  arbitrarily small, tangent or undersampled regions can still disappear.
  Refinement and subsequent clipping enforce the ordinary 262,144 vertex and
  triangle budgets per mesh, returning an error rather than silently dropping
  detail. `Generate` rejections include copied diagnostics on `EngineCallException`;
  catching that operation error allows the product callback to continue and keep
  its prior scene. Backend panics still fail the callback.
  Smaller spacing increases surface sampling and triangle cost; use
  bounded authored pieces. Cuts preserve the source
  surface and interpolate its existing normals/UVs instead of creating shading
  creases. First-region precedence remains unchanged. Added triangles count
  toward the ordinary mesh admission limits. Texture filtering/wrapping comes
  from ordinary materials; material groups remain indexed ranges.

Create an appearance with `engine.Graphics.CreateMeshAppearance(mesh)` and
include it in the product's complete appearance snapshot. A mesh may have
multiple appearances. Dispose appearances before their mesh, and materials
only after meshes using them have been released. The field may be disposed as
soon as generation completes: the mesh owns its copied result. Generate a new
mesh for explicit whole-region replacement.

For collision, `StaticMeshAsset` accepts `new MeshResourceReference(mesh)` in
its `MeshResource` field. Pass that asset and its instances to the existing
`Spatial.ReplaceCollision`, with empty raw vertex/triangle arrays when every
asset uses a reference. Spatial copies the geometry during admission; its
collider remains valid after the source graphics resource is released. Visual
and collision replacement are explicit independent product actions. A zero
reference retains the existing borrowed-array collision path.

`ReadGeneration(field)` reports the most recent successful generation's vertex,
triangle and material-group counts, actual sample spacing, octree depth,
elapsed service time, and reorientation/degenerate facet counts. Generation is
synchronous in the normal product callback; use load-time or explicit bounded
regeneration, not every frame. Partition large authored compositions and give
simple planar solids coarser sampling. Caller-selected output limits are checked
after extraction and do not bound peak memory or guarantee a latency deadline.

The backend uses Fidget 0.5 evaluation and dual-cell connectivity. Engine
triangulates its ordered cell-vertex polygons, avoiding folded fans around
sampled edge intersections at adaptive transitions. Shared-edge winding is
preserved; the retained reorientation counter is zero because individual faces
are no longer flipped against centroid gradients. Zero-area triangles are
omitted. The vendored `fidget-mesh` patch recovers finest-cell QEF vertices
that escape their cell to the mean of that cell's Hermite crossings, and prevents
those invalid solutions from driving collapse. `BoundedLeafVertices` counts these
adaptive recoveries. This bounds placement; it does not guarantee thin-feature
survival or self-intersection-free output. The patch and its source/license ship
in the runtime pack's `share/third-party/fidget-mesh` directory.
Large retained replacements remain ordinary deltas. The host encodes the actual
batch and fragments it for delivery. Reconnect history ages out whole publications,
so later progress pulses cannot remove a large transfer's prefix. Worker timing
samples wait behind earlier output when delivery backs up. Complete committed snapshots still serve fresh connections and recovery;
size alone does not replay/reconstruct the scene or re-enter product callbacks.

Implicit generation readouts also report boundary, non-manifold, and inconsistent
winding edges on extracted geometry before normal, UV, and material splitting.
An entrance into a carved solid can still have a closed rock surface around its
rim. These report-only counts diagnose index topology, not self-intersections,
feature survival, or final attributed-mesh watertightness. The adaptive path
[preserves distinct contour arcs on ambiguous faces](implicit-topology-diagnosis.md)
(Engine #7879). Zero boundary edges alone still does not establish manifold output;
vertex links and self-intersections are separate properties.


### Managed authoring vocabulary

`Rusty.Engine.Implicit` provides optional ordinary C# helpers. `ImplicitRecipe`
scopes an Engine field and composes typed nodes; `RecipeWriter` emits borrowed
`RecipeSurface` descriptions synchronously. The receiver generates and publishes
the mesh before the recipe is disposed. `ArchitecturalRecipes` supplies tunable
layered walls, masonry courses, passages, chambers, joins, and enclosed carving
with explicit portals. Products retain layouts, seeds, materials, artistic
choices, stage ordering, and publication policy. These helpers do not own a
renderer, evaluator, scene registry, or serialization format.

### Retained sampled densities

The same `ImplicitSurfaces` service owns `SampledVolume`. Create one with an
origin, positive uniform spacing, lattice-point counts (at least two per axis),
and initial scalar value. Values are finite floats, negative inside, in x-fastest
order: `((z * height) + y) * width + x`. The last sample lies at
`origin + spacing * (dimensions - 1)`. The retained limit is eight million points.

`WriteSampledVolume` replaces a bounded contiguous sample range;
`ReadSampledVolume` returns a managed copy and descriptor/revision. The generated
lease is released before returning. `SampleSampledVolume` trilinearly samples
inside the explicit domain and rejects outside positions. `RasterizeSampledVolume`
evaluates an analytic field onto the lattice in native batches, committing only
when all samples succeed. Successful writes and rasterization advance the
revision and invalidate the last generation readout; previously generated mesh
resources remain independent snapshots. Disposing the source field does not
invalidate stored densities.

`GenerateSampledVolume` extracts the selected isovalue through Engine's existing
uniform dual-contouring mesher. Its `Field` supplies only optional material
regions, independently of geometry. It reuses analytic generation's attribute,
material, resource, and collision paths. `SampledRecipeSurface` is an optional
synchronous description for this extraction. `ReadSampledVolumeGeneration`
reports actual lattice spacing and topology; octree depth and adaptive leaf
recoveries are zero because this path is uniform. Expected operation errors
return copied diagnostics without poisoning an otherwise handled product call.

Sampling resolution remains a real limit: crossings hidden between lattice
points are lost, and one vertex per active cell cannot represent arbitrary
within-cell topology. The mesher does not add padding or caps at volume edges;
include exterior samples around closed solids. Dense extraction has explicit
cell, temporary-memory, and mesh budgets, so not every retained volume can be
meshed in one request. Partition large products deliberately. This foundation
does not implement erosion, world streaming, or efficient sparse edits.

For opt-in FPS controls and shared human/agent targeting, see [controller and interaction composition](controller-interaction.md).
