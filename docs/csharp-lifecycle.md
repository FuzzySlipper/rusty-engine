# C# product lifecycle and services

The product lifecycle and the Engine services a product calls during it, plus value and native-lifetime rules. Entry page: [C# SDK guide](csharp-sdk.md).

## Lifecycle

The generated `IEngineProduct` contract has lifecycle callbacks for `Start`,
`Update`, `Pause`, `Resume`, `Restart`, `Shutdown`, and `Dispose`. Its optional
`CompleteTimeline` callback receives copied completion data and lets product
code accept or reject the product-owned ticket meaning.

- The constructor receives `ProductCreateContext`, including `IEngineContext`,
  admitted product content, and input configuration.
- `Update(ProductUpdate)` receives Engine-owned update facts and copied input
  events, then returns `ProductUpdateResult` when it needs a supported host
  action.
- `ProductCreateContext.Debugging.Snapshot` holds the latest lifecycle state
  and runtime binding, including host-only fault and control transitions, and
  the facts of the last delivered update.
- The runtime, not the product, drives lifecycle transitions and owns host
  integration. Do not create another central game loop or advance Engine time
  yourself.
- The runtime delivers a timeline completion only when it names the current
  running product binding. Correlation, outcome, and provenance values arrive
  as copied managed data. A product that does not own timeline tickets may
  leave the default rejecting implementation in place.

A product callback is not a transaction: each Engine operation takes effect
when it returns, and an exception rewinds nothing. See
[architecture](architecture.md) for fault handling.

`IEngineProduct` has no attach or reconnect callback. The Engine rebuilds its
renderer from committed state without calling the product, so publish current
presentation during ordinary lifecycle calls and updates. See
[reconstructible presentation](architecture.md#reconstructible-presentation)
for what a rebuild keeps and what it does not replay.

`IEngineContext` has one property per named service family in the ABI's
`NativeEngineApi`: `Input`, `ImplicitSurfaces`, `Diagnostics`, `Dynamics`,
`Motion`, `Kinematic`, `Spatial`, `Perception`, `WorldOrigin`, `Voxel`,
`VoxelContent`, `VoxelScenePresentation`, `Content`, `AuthoredContent`,
`Graphics`, `Presentation`, `Animation`, `Audio`, `Video`, `RenderOutput`,
`CameraView`, `Random`, `Persistence`, and `Ui`. The exact method set is the
generated `Rusty.Engine` output. Mechanics, resolution, and state machines are
ordinary managed helpers, not native context services. See the
[capability map](csharp-capabilities.md) and do not assume a Rust API is
callable from C# simply because its crate is public.

### Particle bursts

`Presentation.EmitParticles` does not need signal registration, an appearance,
or a retained emitter. `SignalId` is a non-empty product label and may repeat;
each call is its own burst. `LogicalId` is for retained emitters and is ignored
by the one-shot call. A minimal valid cube burst inside a product callback is:

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
explicitly. Each curve needs at least two keys with finite ages strictly
increasing from 0 to 1; sizes must be finite and nonnegative, color channels
in 0–1. Lifetimes must be positive and ordered, velocity bounds ordered and
finite, and acceleration finite. A burst needs `0 < BurstCount <= MaxParticles`
and `Seed <= 9007199254740991` (53 bits).

`SizeCurve` values are edge lengths. A cube particle's is in metres. A
billboard's follows `SizeMode`: `Screen` (the zero default) keeps a fixed size
on screen at any distance, 24 CSS pixels per unit, and `World` takes metres and
shrinks with distance like a sprite, which suits world effects such as embers or
dust.

For billboard smoke, use an admitted `Graphics.OpenResource` image handle as
`Sprite` and set `SpriteFrameCount = 1`. More frames require a positive
`FlipbookFramesPerSecond`; cubes require zero. `HasCollision` enables the
explicit collision material and spawn-relative plane/AABB volumes; set
`Collision.LimitBehavior` and each volume's `Kind`. Collision is cosmetic and
does not mutate Spatial or Dynamics.

An invalid descriptor raises a named `EngineCallException`. Catching it lets
the callback continue and publish its other output. Letting an exception
escape faults the lifecycle (see
[Diagnosing native service refusals](csharp-product-project.md#diagnosing-native-service-refusals)).
All emitters and bursts share a retained budget of 4,096 particles; a valid
burst returns `Admitted`, `Clamped` or `Dropped` against it.

A retained emitter (`CreateEmitter`, `UpdateEmitter`) spawns `RatePerSecond`
particles each second; the rate must be finite and not negative. Rate 0 pauses
it: it keeps its handle and its `MaxParticles` reservation, spawns nothing, and
its live particles finish their lifetimes. `Visible = false` also stops it
spawning. Destroy it to give back its reservation.
The [packaged fixture](../fixtures/csharp-particle-emission/ParticleEmissionChecks.cs)
exercises caught refusals followed by cube, billboard and colliding debris
admission in the same callback.

### Bounded dynamics ropes

`Dynamics.SetFixedTether` and `SetBodyTether` attach a caller-selected ID to
world/body-local endpoints in one `DynamicsWorld`. Initial attachments outside
the maximum distance reject. Updating the same attachment changes its target;
its effective length approaches that target at the supplied rate. `ReadTether`
reports effective/target length, distance/slack, caught/taut state and a
sampled force proxy in N. `RemoveTether` returns a release receipt and
preserves body velocity. A removed body invalidates its attached tethers.

`CreateFixedChain` and `CreateBodyChain` own one anchored end and a series of
sphere beads; the final bead is free. Supply ordinary body properties and an
initial end position. Engine spaces the initial beads along that segment and
creates all bodies and links; a refused chain leaves none behind.
`ReadChainPoint` returns the anchor at index zero followed by bead centers in
order. `SetChainLength` distributes total length and reel rate evenly across
links. `RemoveChain` removes all its bodies and links; destroying a body anchor
invalidates and removes the chain. Adjacent bead contacts are suppressed.
Ordinary collision groups select nonadjacent self-collision; terrain collision
uses the same dynamics scene as other bodies. The segments between beads have
no collision geometry.

Defaults are four subdivisions and eight solver iterations per supplied tick;
`ConfigureRopes` selects others (each at least one). `Dynamics.Step` reports
actual rope link/subdivision/iteration work over the requested ticks.
Higher counts cost more work; they do not create a new update clock. Preserve
ordinary body CCD/sleep policies. Fixed endpoints follow `WorldOrigin` rebasing.
Force readouts sample terminal solver-substep impulses, so they are not peak
catch loads or a breakage policy. See the [physics contract](rope-physics.md).

### Composing graphics

Use `context.Graphics` for resources and retained facts; `Appearance` names a
selected visual resource. `AppearanceFact` carries `ObjectId`,
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

Either call may list parents and children in any order; the Engine checks the
hierarchy against the state after the whole batch and creates parents first.
A removed object's children must be removed or moved in the same batch. A
refused call changes nothing. Changing an appearance in place, such as its
materials or sprite frame, updates the objects showing it without republishing
them. `EntityGraphicsProjection` also accepts an optional parent `EntityId`.

A light created with `Graphics.CreateLight` and a parent object shows while
that object is in the published scene. Create it before or after the publish
that first shows its parent, in the same callback. A publish that removes the
parent takes the light out with it, and a later publish that brings the
parent back shows the light again. Dispose the light when the product is done
with it.

- Attached equipment: publish the actor and equipment as ordinary facts, with
  the equipment parent naming the actor. Product code selects equipment and
  local placement; Engine owns hierarchy and resource realization.
- Target indicators: compose child sprites/meshes with anchored billboards;
  choose depth-tested, occluded, or always-on-top layers through `Presentation`.
- Procedural effects: combine admitted meshes/materials with lights and
  retained emitters or explicit bursts. `Graphics.CreateMeshResource` also
  admits runtime-generated triangle streams as a disposable Engine resource.

`PrimitiveGeometry.Line` creates an ordinary retained line from local
`(0, 0, 0)` to `(0, 1, 0)`. Set its appearance translation to the first
endpoint, rotate local +Y toward the second, and scale Y by their distance.
Publish it as an ordinary appearance fact; Engine owns line realization and
cleanup.

### Look and controller input

Look math is `Look.Integrate(request)` (and `Reset`, `Rebase`, `Diagnose`) in
the managed toolkit. Use `Look.IntegrateClamped` for interactive pointer/stick
input: it saturates large finite angular deltas at the configured limit so a
quick mouse turn does not throw out of the product update. `Integrate` rejects
a delta beyond that bound.

Controller sticks arrive as `ControllerAxis` physical input with the normalized
value in `ProductInputEvent.X`. Analog buttons (including standard-gamepad
triggers) arrive independently as `ControllerButtonValue`, with `X` in `[0, 1]`;
`ControllerButton` carries digital press/release edges. Products can map
`controller-button-value:button-7` to an `axis` intent for proportional right
trigger input, just as `controller-axis:axis-0` maps the left stick's X axis.
The runtime retains these scalar values between samples and clears them with
the input lane. Products own dead zones, movement meaning, and stick look speed;
integrate a held stick's angular rate using admitted simulation time, rather
than treating each input sample as a mouse displacement.

### Runtime-generated geometry

Ordinary `MaterialRequest` exposes opaque, mask/cutoff, and blend alpha modes,
and `Metalness` from 0 (the default, dielectric) to 1 (metal; see
[the standard shader](lighting-and-sky.md#the-standard-shader)).
`NormalMap` names a tangent-space normal map (x right, y up the image, as
glTF stores them), and `NormalScale` scales its tilt. The texture must be
opened as data, with `TextureColorSpace.Linear`. A colour texture is refused
with `CSHARP_MATERIAL_NORMAL_MAP`. The map reads the base texture's uv. On a
voxel surface it repeats or follows the atlas region with the base texture,
and its tangent frame stays continuous across tile seams.
`TriplanarSharpness` (0, the default, for none) samples the base texture and
normal map from three axis planes blended by the surface normal instead of
the uv; see [textures on reconstructed
surfaces](smooth-voxel-surfaces.md#textures-on-reconstructed-surfaces).
`Shader` names a product shader opened from a `.wgsl` file (its keywords
chosen at open with `RenderResourceRequest.ShaderKeywords`), with four
parameter vectors and two textures of its own; see [product
shaders](lighting-and-sky.md#product-shaders).
`AuthoredMaterialAppearanceRequest.Shader` does the same for authored and
voxel materials.
Sprite requests accept a `SpriteMaterialDescriptor` for lighting, normal/depth
maps, alpha, and shadow policy. Sprites and atlases keep the sampler selected
when their texture was opened; the short constructors select opaque mesh and
unlit/blended sprite defaults.

`Graphics.CreateMeshResource(new MeshResourceCreateRequest(positions, normals,
uvs, colors, indices, groups, bindings))` copies ordinary managed arrays into
an immutable retained mesh. Positions/normals are `Vector3`; optional UVs are
`Vector2`; optional vertex colors are linear `Color` RGBA; indices are `uint`.
`MeshGroup` ranges tile the triangle index list, and `MeshMaterialBinding`
selects an existing Engine material for every used slot. Bounds are computed by
Rust. The mesh needs at least three vertices, matching normal and optional
UV/color streams, complete triangles, and nonempty groups and bindings. Vertex
and index counts are stored as `u32`; there is no other size cap.
`GraphicsMeshLimits` describes only the managed span representation. Packed
mesh resources use `u32` byte lengths and offsets.

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

Remove appearances from the published set before disposing them, then
dispose their mesh resource. Dispose bound materials after their resources and
appearances. The Engine releases unused mesh definitions and GPU geometry.
See the [procedural mesh fixture](../fixtures/csharp-mesh-composition) for a
C# shockwave composed from these primitives.

### Retained camera composition

`CameraView` retains cameras, offscreen targets, and a complete typed
`CameraCompositionRequest`. A composition view selects a live camera, its
normalized viewport, ordering, and either the primary surface or one Engine
target. Presentations copy an offscreen target into a normalized primary
destination, so split-screen and an inset/rear view remain Engine-rendered.
Targets' GPU lifetime belongs to the renderer; C# must not use a target as an
arbitrary mesh texture. `SetActiveCamera` is the single-primary-view
convenience over this same retained composition. Use `CameraViewports` for
ordinary full, split, and inset normalized rectangles. A composition with any
primary view owns the primary surface: the renderer draws no separate
default-camera pass, and area outside every primary view is left cleared.

`SetBackgroundColor(new(new Color(r, g, b, 1)))` selects an opaque retained
viewport clear color and replaces any selected sky. `SetSkyBackground` replaces
that color with a retained panorama; `ClearSkyBackground` returns to the Engine
default. Products choose the color or resource while the Engine owns renderer
state and realization. `SetSkyBackgroundBlend` blends two retained panoramas
from a product-supplied value without rebuilding resources; see
[lighting and skies](lighting-and-sky.md) for clock composition and voxel
direct-light sampling.

`UpdateCamera` applies immediately. Opt into render-time sampling with
`UpdateCameraSample(new CameraSampleRequest(camera, descriptor, sampleTimeSeconds,
delaySeconds, CameraInterpolation.Position, cut))`. Use the admitted simulation
facts for a monotonic sample timeline; a useful end-of-batch timestamp is
`(facts.SimulationStep + facts.AdmittedStepCount) * facts.FixedDeltaSeconds`.
`Position` interpolates translation while using the latest published orientation;
`Pose` also interpolates orientation, including explicit-basis roll. `Latest`
returns to immediate presentation. A one-step delay is a starting point, not an
Engine-wide policy. In position mode, look is still limited by its publication
cadence: this API does not predict input.

Pass `cut: 1` on teleports, origin rebases, and discontinuities; ordinary samples
use `cut: 0`. A cut, a sample time that does not advance, and an
interpolation-mode or delay change discard history. Republishing the same
sample adds nothing.
Missing samples hold the latest pose without extrapolation. If arrival time
advances more than the source timeline by twice the configured delay, the
sample resets the clock mapping, so a paused source clock cannot permanently
disable interpolation. The renderer keeps at most 64 recent samples per camera;
a delay requiring older history holds the oldest available pose. The renderer
maps the product timeline to its local clock; these times are not
cross-process latency measurements.

This is presentation only. Gameplay rays and selection continue to use the
product's authoritative camera. A delayed image may therefore differ from a
current gameplay hit, especially while moving close to objects. Products choose
whether this tradeoff is appropriate. The runtime's renderer readout reports
the cameras actually drawn separately from the authored `sourceCameras`.

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

`Graphics.OpenResource(new RenderResourceRequest(path))` selects nearest
filtering and clamp wrapping for PNG textures. Ordinary tiled meshes can select
sampling explicitly:

```csharp
var texture = engine.Graphics.OpenResource(new RenderResourceRequest(
    "textures/stone.png", TextureFilter.Nearest, TextureWrap.Repeat));
```

`TextureFilter.Linear` is also available. A fourth argument,
`TextureColorSpace.Linear`, opens a PNG as data rather than sRGB colour; normal
maps need it. The same PNG opened both ways is two retained textures. A linear texture gets a mip chain
at upload, built in linear light, and materials sample it trilinearly with
16× anisotropic filtering, so tiled surfaces do not speckle at distance. That
costs about 40 ms and a third more memory for a 2048² colour texture.
Voxel-surface tiling picks its level from the unwrapped tile coordinate,
clamped so an atlas region keeps at least 4×4 texels. Nearest textures stay
single-level, for pixel art. Sky panoramas, sprites and particles always
sample the base level. GLB textures follow their sampler's `minFilter`.
Sampling applies only to PNG resources. The same PNG can be selected with different samplers: pixel content
is shared, while each sampler has its own retained texture identity. Keep
sprite/atlas resources clamped unless their authored usage calls for something
else.

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
objects: source presence and match, whether a renderer observation exists,
fallback/limitation facts, current sector and offset, the effective
capture/configuration, and retained resource counts/timing when the renderer
reports them. The capture freezes the source Appearance pose, so this is a
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
it needs no voxel-specific shaders or product rendering code. It remains
bounded by source, dimension, voxel, frame, mesh, and material limits, and it
is not a general scene import path. Unsupported source or presentation needs
are an upstream Engine task and a valid stopping point.

`VoxelScenePresentation` projects the canonical `Spatial` session voxel scene
through the Engine renderer. Bind every currently used scene material slot to
a live `Appearance` material, retain the disposable projection, and call
`RefreshScene` after voxel edits, residency changes, or origin changes. A
refresh right after one voxel update visits only the chunks that update
changed; after several updates, a replaced scene, a material change or an
origin rebase it checks every chunk. `ProjectSceneDirectional` and
`UpdateSceneDirectional` additionally accept sparse `SpatialFace` overrides;
omitted faces use the required base slot binding. On a reconstructed surface
the face is the polygon's box-projection face. `ReadMaterialMapping` returns
copied effective source-slot/face selections, material provenance values, and
renderer slots. The Engine keeps incremental renderer identity and owns all
generated mesh/frame work; C# receives only copied facts. `Clear` or disposal
removes the matching renderer objects. `SpatialSessionConfig.VoxelSurfaceMode`
selects how the session's voxels are drawn; `Voxel.ConfigureMaterialSurfaces`
changes it, and each material's own mode and character, later. See
[smooth voxel surfaces and densities](smooth-voxel-surfaces.md).

### Voxel material collision

Immediately after creating a Spatial session, call
`Voxel.ConfigureMaterialCollision(new(session, declarations))` with
`VoxelMaterialCollision(slot, collidable)` values from the product's material
definitions. Unlisted slots collide, which is the default occupied-cell
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
throws an `EngineCallException` carrying the native reason (for example
`unresolved-character-controller-penetration`, including remaining depth).
The product decides whether to prevent such edits, relocate the actor, or
otherwise handle the rejected motion. Edit batches have no cell-count limit;
the fixture exercises 64-cell batches and subsequent updates.

### Voxel scene material palettes and atlases

A `GreedyCubes` scene can bind materials from multiple authored voxel atlases,
including distinct atlases over the same texture. Atlas identity, region,
texture and alpha mode belong to each material; there is no scene-wide atlas.
Create each material with `Graphics.CreateAuthoredMaterial` and its selected
texture resource, then bind its source slot through `VoxelScenePresentation`.
`AuthoredMaterialAppearanceRequest.NormalMap` adds a normal map opened with
`TextureColorSpace.Linear`. It repeats with the material's tiling, or
shares an atlas material's region layout, so an atlas normal map matches its
colour atlas texel for texel. `TriplanarSharpness` blends the material's
texture from three planes on smooth surfaces
([textures on reconstructed surfaces](smooth-voxel-surfaces.md#textures-on-reconstructed-surfaces)).
An atlas reference must keep the catalog's pinned version/hash. Structural
class does not select a scene atlas or replace canonical voxel state.

Base bindings must cover every currently meshed source slot, with no duplicate
slots. The retained palette may also contain slots not currently meshed, so
adding/removing voxels or streaming chunks does not require trimming the palette.
Face overrides require a corresponding base binding, even when that slot is
currently absent. `RefreshScene` uses the retained palette; use `UpdateScene`
or `UpdateSceneDirectional` when adding a new material binding. `MaterialCount`
counts the retained base palette, including currently unused slots.

Source slots are unsigned 16-bit identities (0–65,535); base and face bindings
share 65,536 renderer slots across retained voxel presentations. Out-of-range
source bindings name the slot, material handle and limit; renderer-slot
exhaustion names the binding and total capacity in the typed operation
diagnostic. The [sixteen-material fixture](../fixtures/csharp-voxel-capacity)
loads one atlas, renders sixteen base materials in one scene, rejects source
slot 65,536 and successfully refreshes afterward.

Projection, refresh and material-update failures throw `EngineCallException`.
Inspect `Service`, `Operation` and `Diagnostics.Span` for the Engine code and
message (for example, the missing source slots), rather than just the numeric
status. A caught binding rejection leaves the existing projection usable.
The packaged [two-atlas fixture](../fixtures/csharp-voxel-atlases) exercises
solid/decorative and opaque/blend materials, unused palette entries, and caught
rejections followed by successful refreshes.

### Spatial navigation

`Spatial.EvaluateNavigationStep` evaluates one bounded planar-navigation step
against the retained session projection and returns the typed outcome, next
waypoint and path cell, and the whole path it follows in `Path`. It does not
change revisions, projections or other session state. The path requests
(`RequestNavigationPath`, `RequestWeightedNavigationPath` and their volumetric
forms) also return their cells in `Path`; the session keeps no path of its own,
so a product that needs a path later keeps the one it was given.

`Spatial.ReplaceCollisionNavigation` derives and retains a planar projection
from the session's current Engine collision scene. The request supplies a
finite `WorldMin`/`WorldMax` volume and a `CollisionNavigationConfig`; it never
supplies geometry. The Engine samples voxel and retained static-mesh collision
for support and headroom, preserves supported elevations, and checks a
standing capsule at each cell center. Directed connections between supported
cells use the character collision capsule casts and step solver: a traversable
floor lip can connect without admitting a thin separating wall or insufficient
headroom. Path, weighted-path and navigation-step queries apply these edge
checks alongside product traversal overlays. This is a bounded route
suggestion only: normal character collision and controls remain the authority
for physical movement. Product door and hazard state belongs in the planar
traversal overlay; read-only evaluation honors that overlay.

Start from `Spatial.DefaultCollisionNavigationConfig()` and change one option
at a time with `with`:

| Option | Meaning | Default |
|---|---|---|
| `GridId`, `CellSize`, `ChunkSize`, `MaximumCells` | The world-aligned grid and the most X/Z columns one publication may cover. | 1, 1 m, 16, 65,536 |
| `Character` | The body navigation stands and moves with. Pass the `CharacterControllerConfig` the product's character uses: `Shape` radius, standing height and contact skin; `Surface` maximum slope, maximum step height (metres up), minimum step width and floor snap; `Recovery` tolerances. Its radius may be at most half a cell. | `DefaultCharacterControllerConfig()` |
| `MaximumDrop` | Farthest a one-way downward edge falls, in metres. | The default step height |
| `VerticalSearchCells` | How many cell levels up or down a neighbouring support is searched for. The step height and drop decide which of those are edges; raise this with them, since a support beyond it is never a neighbour. | 1 |
| `SupportsPerColumn` | Surfaces sampled per X/Z column, top down. A deeper layer is unknown, not walkable. | 8 |
| `DiagonalNeighbors` | Also connect diagonal neighbours. A diagonal is one path step, as an orthogonal one is. | off |
| `SnapAbove`, `SnapBelow` | How far a query point may lie above or below a support and still stand on it. | 0.101 m |
| `SnapAcross` | How far beyond the footprint of the cell containing a query point a support may be taken from. | 0 (that cell only) |

Each X/Z column is sampled down through solid as well as air,
so floors inside an enclosed volume (rooms stacked in rock) are found. On a
slope the capsule's feet rest above the surface, by `(radius + skin) × (1/cos
θ − 1)`, so any slope up to the maximum is a support. A riser up to the step
height is climbed when the headroom above it fits the body, as the character
controller climbs it. A slope up to the maximum is walked up and down whatever
the step height and drop: an edge steeper than a step or longer than a drop is
admitted when the ground between rises or falls no more than such a slope over
any short stretch, so a riser or a cliff of the same grade is still refused.

To see why navigation hangs up, ask it:

- `ExplainCollisionNavigationColumn(session, x, z)` samples one column again
  against current collision with the published configuration. Each sample is
  a surface the downward casts hit, with its height, normal, where the feet
  would stand, its cell, and an outcome: `Support`, `TooSteep`,
  `CapsuleOverlap` (with the collision chunk or static-mesh instance and the
  contact point), or `SameCell` (a higher support holds that cell). No samples
  means nothing was hit; `BudgetExhausted` means a surface remains below the
  last one sampled.
- `ExplainCollisionNavigationEdge(session, from, to)` evaluates one directed
  edge: `Traversable`, `FromNotSupport`, `ToNotSupport`, `NotNeighbor`,
  `RiseOverStep`, `DropOverMaximum`, `StartOverlap`, `EndOverlap`,
  `HorizontalSweepBlocked`, `DescentBlocked` or `StepManeuverFailed` (or,
  with jumps on, `JumpTraversable`, `RiseOverJump`, `JumpHeadroomBlocked`,
  `JumpArcBlocked` or `GapTooWide`), with both support heights and whether
  the installed navigation holds the edge.
- A `NoPath` step result carries `Visited` and the visited cell nearest the
  goal (`NearestCell`, standing at `Nearest`).

Use the live foot position for `EvaluateNavigationStep` so it can reconcile to
the nearest retained support (see below).

**Jumps.** Navigation walks by default. Two configuration switches add jump
edges, using the body's own jump from a standstill at the start's column
centre: it leaves the ground at `Character.Vertical.JumpSpeed`, capped at
`TerminalRiseSpeed`, under `Gravity`, peaking `speed² / 2 Gravity` above the
start, and falls back no faster than `TerminalFallSpeed`. It lands as it
falls to the target, or to `Surface.FloorSnapDistance` above it when falling
no faster than `FloorSnapSpeedLimit`. It holds toward the target as late as
still lands it there, gaining speed as the controller does in the air,
`Air.Acceleration × LateralControl × wish` each second, where the wish is
`Ground.ForwardSpeed` capped by `Air.WishSpeedCap`, up to `Air.MaximumSpeed`
(`Air.Drag` is not modelled). A mover that stops at the start, jumps and
holds toward the target from no later than that makes the jump.

- `JumpLedges` connects a neighbour too high to step onto when the capsule can
  rise straight up to the peak (`JumpHeadroomBlocked` otherwise) and sweep
  clear over to it (`JumpArcBlocked`). The rise must stay below the peak less
  the contact skin and recovery nudge (`RiseOverJump`).
- `JumpGapCells` connects a support straight along X or Z, and with
  `DiagonalNeighbors` also diagonally, across up to that many columns. Those
  columns must hold no support within `VerticalSearchCells` of the start, and
  the jump must carry the distance (`GapTooWide`), sweep clear
  (`JumpArcBlocked`), and land no farther down than `MaximumDrop`.
- Path, weighted path and step results list each path cell's `Edges` (`Walk`,
  `Drop` for a fall below the step height, or `Jump`). A step result's
  `NextEdgeKind` tells a mover to jump now.
- A weighted path pays `JumpCost` (default 4) on top of a jump's destination
  cost, so it prefers a walk unless the detour is longer.

Republishing is incremental. The session keeps the previous collision-derived
publication's columns and connections, and a new one derives only the X/Z
columns that are new to the box or lie within one cell of collision that
changed since: voxel edits and voxel chunk residency record where they
changed, and a world-origin rebase by a whole number of cells moves the kept
columns with the grid. Any other collision change (static-mesh residency,
collidable materials, content artifacts), a different policy, or a different
`WorldMin.Y`/`WorldMax.Y` derives the whole box. So keep the vertical range
fixed, or snap it to a coarse step, when the box follows a moving point. The
receipt's `DerivedColumnCount` and `ReusedColumnCount` show which path ran;
the result is the same as a full derivation. A republish updates the installed
navigation in place, so its cost follows the derived columns and their
neighbours, not the box: one that derives nothing costs microseconds.

The receipt also reports the cost: `EdgeTestCount` is the support-to-support
edges tested with capsule casts, and `DerivationMicroseconds` the time the
publication took. Columns and edges are derived across the machine's cores.
Reconstructed surfaces cost more than cubes because each capsule cast meets
triangles rather than boxes, and rough facets send more level edges through the
step solver.

### Collision navigation coordinates

`ReplaceCollisionNavigation` uses a **world-aligned grid with origin (0, 0, 0)**
in the session's coordinate frame at publication. `WorldMin` and `WorldMax`
select where collision is sampled; they do not translate the grid. `ChunkSize`
groups cells and `GridId` identifies the grid; neither changes its origin.
`RequestNavigationPath`, `RequestWeightedNavigationPath`, traversal cells and
returned path cells use these same signed coordinates. Every navigation source
(`ReplaceNavigation`, `ReplaceVoxelNavigation`, `ReplaceCollisionNavigation` and
a content artifact's navigation) keeps its grid this way, and a
`WorldOrigin.Commit` moves the installed grid with the world as it moves
collision, whether or not the move is a whole number of cells. Its cells, supports, traversal
overlays and connections are unchanged; positions, both those a query accepts
and the waypoints and nearest positions it returns, are in the current local
frame. So after commits whose receipts moved local coordinates by a total `d`
(origin before minus origin after, summed), the cell holding local position
`p` is the one that held `p - d` at publication. A later publication, or a
content artifact replaced after the commit, starts again at (0, 0, 0) in the
current frame.

For cell size `s`, a retained support at world position `(x, supportY, z)` in
the publication's frame has:

```text
cell.X = floor(x / s)
cell.Y = floor(supportY / s)
cell.Z = floor(z / s)
```

Use mathematical floor, including for negative values. The level is the
**collision surface's support height** (on a slope, the height the feet stand
at), not the occupied floor voxel's index,
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

For creature movement from live foot positions, prefer
`Spatial.EvaluateNavigationStep(new NavigationStepRequest(session, fromFeet,
targetFeet, maximumStepDistance, maximumVisitedCells))`. It resolves each
position to the nearest retained support within the configuration's snap:
`SnapAbove`/`SnapBelow` vertically and, beyond the containing cell's
footprint, `SnapAcross`. The nearest across wins, then the nearest vertically.
The defaults (0.101 m, and the containing cell only) handle the controller's
small standing clearance. A position with no support within the snap returns
`StartNotWalkable` or `GoalNotWalkable`. Supply both endpoints in the session's
current local frame.

The result's `NextPathCell` is a grid identity; `NextWaypoint` is a
world-space movement proposal bounded by `maximumStepDistance`. Use that
proposal with ordinary character collision. For an intermediate path cell, its
X/Z center is `((X + 0.5) * s, (Z + 0.5) * s)`, but its level only identifies a
height interval: the retained support can be sloped or fractional. The Engine
uses that support height when constructing its proposal.

The [packaged mapping fixture](../fixtures/csharp-navigation-mapping/NavigationMappingChecks.cs)
checks positive and negative coordinates, non-unit cells, unaligned publication
bounds, every reported walkable cell, a multi-cell route, and world-position
steering. It also reproduces `StartNotWalkable` from subtracting the box minimum,
and explains a wall the default step refuses before a one-unit step and drop
cross it.
Run it with `scripts/test-csharp-sdk-package.sh --coreclr-smoke`.

### Spatial triggers

Spatial trigger definitions stay registered for the session while their
active state can change. `ReconcileTriggers` and `RestoreTriggers` read the
product's collider rows for that call only: a row whose entity is a registered
trigger is that trigger's world-space AABB, and every other row with enabled
collision is a subject. `SetTriggerActive` deactivation removes current
overlaps and returns their exit facts, while reactivation returns no synthetic
enter—the next ordinary `ReconcileTriggers` observes real geometry and produces
any new edge. `ReconcileTriggers` and `SetTriggerActive` return their
enter/exit edges in `Facts`. `RestoreTriggers` accepts the complete active
trigger ID set plus current projected colliders and replaces the active and
overlap state without producing gameplay facts. `ReadTrigger` returns the
current active flag and every overlap subject. Unknown IDs, activating an
active trigger (or deactivating an inactive one), and duplicate restore IDs
reject without changing the session. Disposing the Spatial session destroys
the definitions, active set and overlaps together.

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

The Engine resolves the immutable reference, parses the schema, and checks
finite bounds, triangle indices and navigation coordinates. It prepares
collision and navigation before changing either. Success replaces all retained
static-mesh assets/instances and planar navigation and clears the traversal
overlay; voxel content and residency are untouched. The returned digest,
revisions, counts and projection hashes—and `ReadContentArtifact`—identify the
admitted source. Disposing the Content reference afterward does not remove the
copied spatial state.

The same facts can also be written as a compact binary file, recognised by
its first eight bytes, `RSPATIAL`. It is admitted through the same call with
the same validation and refusals, so a binary file and the JSON file of the
same facts install the same collision and navigation: the receipt's counts and
navigation projection hash match. Its collision projection hash differs,
because the collider asset is named by the artifact's content digest. Use it for large
navigation sets: a town of 544,563 cells is about 12 MB instead of 43 MB of
JSON, and parses and validates in about 15 ms instead of 75 ms. Write
little-endian, with no padding:

| Field | Encoding |
|---|---|
| Magic | `RSPATIAL` (8 bytes) |
| `schemaVersion`, `navigation.config.schemaVersion` | u32, u32 (both 1) |
| `bounds.min`, `bounds.max` | 6 × f64 |
| `cellSize`, `levelQuantum`, `maximumSlopeDegrees`, `requiredHeadroom`, `supportProbeDrop` | 5 × f64 |
| `staticMeshArtifactId`, `navigation.id` | each a u32 byte length, then UTF-8 |
| position, triangle and cell counts | 3 × u64 |
| `collision.positions` | count × 3 f64 |
| `collision.triangles` | count × 3 u32 |
| cell `column`s, then cell `row`s | count × i32 each |
| cell `supportHeight`s | count × f64 |
| cell `walkable` flags | ⌈count / 8⌉ bytes; cell `i` is bit `i % 8` of byte `i / 8` |

A cell's `level` is not stored: it is `round(supportHeight / levelQuantum)`,
which the JSON form must state and validation checks. Cells written in
(column, row, level) order admit fastest. [`valid.rspatial`](../fixtures/csharp-spatial-artifact/valid.rspatial)
is `valid.json` in this encoding.

Refusal leaves collision, navigation, artifact identity and residency unchanged.
A truncated or otherwise malformed binary file is refused with
`CSHARP_SPATIAL_CONTENT_SCHEMA`.
`EngineCallException` reports service `Spatial`, operation
`ReplaceContentArtifact`, and a named diagnostic such as
`CSHARP_SPATIAL_CONTENT_BOUNDS`, `CSHARP_SPATIAL_CONTENT_COLLISION`,
`CSHARP_SPATIAL_CONTENT_NAVIGATION` or `CSHARP_SPATIAL_CONTENT_SCHEMA`.
An unknown or disposed reference reports `CSHARP_SPATIAL_CONTENT_REFERENCE`.

### Placed artifacts side by side

To stream several precompiled closures (sites, cells, dungeon blocks) into one
session, place them with `Spatial.ApplyContentArtifactResidency`. Each
`SpatialContentArtifactInstance` names an artifact by `ContentReference` under
a stable product `Id`. It turns the artifact `QuarterTurns` times 90° about +Y
around its own origin (a quarter turn takes +X to −Z), then moves it by whole
cells of the navigation grid: `ColumnOffset` (x), `LevelOffset` (y, in the
artifact's level quantum) and `RowOffset` (z). Other angles would take its
cells off the grid, so placements turn by quarter turns only.

```csharp
SpatialContentArtifactResidencyReceipt resident = engine.Spatial.ApplyContentArtifactResidency(
    new(session,
        new SpatialContentArtifactInstance[] { new(siteId, siteArtifact, 64, 0, 32, QuarterTurns: 1) },
        new ulong[] { leavingSiteId },
        7, 8, 1));
```

- **Composition.** Removals apply first, so one request can also replace an
  identity. Each placed artifact's collision joins the session's static
  collision as the static-mesh instance under its `Id`. Planar navigation
  becomes one projection over the base artifact's cells
  (`ReplaceContentArtifact`) and every placed artifact's. Paths cross between
  neighbouring closures as within one. A cell two artifacts both declare is
  walkable, at the higher support.
- **Grid.** All of them must share one cell size and level quantum
  (`CSHARP_SPATIAL_CONTENT_NAVIGATION_GRID`). The grid is the base artifact's,
  or, with no base, the local frame when the first artifact was placed. A
  world-origin commit moves every placed artifact with it, and later
  placements land on the same grid.
- **What else stays.** Removing one placement leaves the others, the base
  artifact and collision added through `ApplyCollisionResidency`.
- **Ending the composition.** `ReplaceContentArtifact`,
  `ReplaceCollision`, `ReplaceNavigation`, `ReplaceVoxelNavigation`,
  `ReplaceCollisionNavigation` and `ClearNavigation` end it. Artifact
  collision then stays as ordinary static collision.
- **Refusals.** An `Id` that is already resident, placed or the session's own
  static collision, is refused with `CSHARP_SPATIAL_CONTENT_IDENTITY`.
  `ApplyCollisionResidency` refuses to change a placed artifact's collision
  (`CSHARP_COLLISION_CONTENT_OWNED`). Any refusal, including a malformed
  artifact, changes nothing.
- **Lifetime.** Placed artifacts with the same content share one collider
  asset, released with the last of them. The content references are read
  during the call and can be disposed afterwards.

Products own generation recipes and artifact semantics: required connected
regions, portal/socket pairing, keys, gates and provenance policy. The Procgen
floor document is a product/generator format and must be converted to this
Engine format before admission. Spatial accepts precomputed navigation facts;
it does not rederive support or enforce that every region connects. For
navigation derived from live collision, use `ReplaceCollisionNavigation`.
Content supplies the artifact byte digest; `Content.ResolveReference` can select
an expected path/digest before spatial admission. Spatial does not interpret a
Procgen payload hash. These semantic checks belong in the generator/importer or
product before this call.

The [packaged C# fixture](../fixtures/csharp-spatial-artifact/SpatialArtifactChecks.cs) verifies
collision ray hits, navigation, named extent rejection, unchanged residency and
successful admission after a refusal via `scripts/test-csharp-sdk-package.sh
--coreclr-smoke`.

### Character steps

`CharacterStepRequest.Obstacles` is a borrowed, call-local list of active
product-authored colliders. Give each obstacle its stable entity identity,
current transform, local bounds, collision participation, and motion facts;
the Engine uses them for the one controller proposal and returns ordinary
`CharacterMotion`, `CharacterSupport`, and platform facts. Resubmit the
current support transform and obstacle list on later steps so Engine-owned
support/carry continuation can apply; the session never retains product
entities or collider records. Collision uses translation-offset AABBs with
unit scale; obstacle rotation participates in platform carry but does not
rotate the collider volume.

For a moving retained static-mesh instance, use the call-local
`CharacterStepRequest.MeshInstances` collection. Each
`CharacterMeshInstance` names the admitted `StaticMeshInstance` by its stable
instance ID, supplies the product entity ID that should receive support facts,
and carries the current linear and angular velocity. The Engine uses the
retained triangle mesh for collision and support, then applies translation and
rotation carry from the retained instance transform. Do not also submit the
same model as a `CharacterObstacle`: the mesh instance is one collision and
support authority. Update its retained pose through `ApplyCollisionResidency`
before the step and resubmit its `CharacterMeshInstance` each step. The
separate `CharacterSupport` value may be absent for mesh support; the Engine
reads its pose from residency. Product persistence keeps the instance/entity
IDs and pose as ordinary values; no native handle is part of the saved state.

### Character tethers

Set `CharacterStepRequest.Tether` with `CharacterTetherRequest.AtFixedAnchor`
or `AtDynamicAnchor`; the default request is untethered. Use a stable
nonzero attachment ID and a character-local point. Maximum length admits the
initial attachment; target length changes at the authored reel speed. Resubmit
the request each step, or omit it to release while keeping accepted momentum.
`CharacterMotion` carries attachment and effective-length continuation alongside
the controlled/external velocity, with no separate swing state.

For a dynamic anchor, call `Dynamics.ObserveAnchor` with its body and local
point before each character step; a disabled body returns an invalid
observation and the character receipt reports invalidation. Engine resolves
point velocity, center of mass, and impulse response including inertia and
locked axes. Product code need not calculate these. The character uses
effective mass and this response to share the velocity correction, capped by
`MaximumDynamicImpulse` on both sides.

`CharacterStepReceipt.Tether` reports endpoints (in character-to-anchor order),
effective maximum length, separation, taut/caught/released/invalidated state,
radial and tangential velocity, swept correction, saturation and unresolved
separation. A dynamic receipt also contains `Reaction`. Apply the chosen
reactions explicitly through `Dynamics.StepWithReactions` together with ordinary
actions; this performs one normal Dynamics step, applying each reaction as an
impulse at its observed point. Applying a reaction twice applies it twice.
Observe anchors, propose character steps, then apply the reactions. There is
no hidden Dynamics step inside the character controller.

Terrain can prevent a length correction, and the impulse cap can leave the rope
extended; inspect `Unresolved` rather than assuming an exact rigid constraint.
Reeling is caller-authorized work, not a promise of energy conservation. Use the
receipt endpoints for ordinary debug-line presentation. The public-facade
exercise is in `fixtures/csharp-nativeaot-trial/Product.cs`.

### Character continuation

To save a character continuation, call `CaptureCharacterContinuation` with the
latest `CharacterStepReceipt.Generation` and persist the copied
`CharacterContinuationCheckpoint` beside the product-owned pose and look. After
recreating a compatible `SpatialSession` and its canonical content, call
`RestoreCharacterContinuation`; use its returned `Motion` and the checkpoint's
`Config` for the next `ProposeCharacterStep`, while supplying current
product-authored support and obstacle facts as usual. Capture rejects a
generation other than the latest step's. Restore rejects invalid motion, a
changed session configuration, changed canonical content, and a target session
that has already stepped a character. A checkpoint is a plain value. Its source
session identity and generation are copied diagnostic provenance, not a native
handle that remains resolvable after save/load.

## Values, handles, and native lifetime

The public C# layer turns direct service calls into typed requests, results,
values, and disposable handles:

- Returned values and exception diagnostics are managed copies; keep them as
  long as needed. The native results they were copied from are valid only
  until the next call on the same service, and the generated wrapper copies
  them before returning.
- `ProductUpdate.Input` is a span valid for that `Update` call only.
- Do not retain request spans, native pointers, or callback data beyond
  their call.
- Dispose values that represent an Engine session, resource, or handle when
  their scope ends. `using` is the usual product-side shape.
- Do not add unsafe code, handwritten P/Invoke, ABI structs, or
  `UnmanagedCallersOnly` exports to normal product code. The generator owns
  those details.

The product boundary is trusted, but these memory and lifetime rules are real
correctness requirements rather than policy ceremony.
