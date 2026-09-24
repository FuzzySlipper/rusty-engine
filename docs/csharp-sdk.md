# C# SDK guide

This guide describes the C# surface that exists today. It distinguishes that
surface from recommended product organization so an agent does not mistake a
proposal for an API.

For world-object use (containers, doors, talk), controller aim assistance, and
agent-friendly testing, start with [world interaction](controller-interaction.md).
`WorldInteraction` shares ordinary actions with explicit target-ID assistance;
`InteractionDebugModule` exposes discoverable `interaction.inspect` / `use` commands.

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

### Runtime input remapping

Use `context.Engine.Input.ReplacePhysicalMappings(mappings)` to replace the
whole physical mapping set during product creation, Start, Pause, Resume,
Restart, or an admitted Update callback. The mappings use the existing
`ProductInputMapping` values and must target the product's declared semantic intents; remapping does not add intents
or change their value kinds or payload contracts. An empty set disables
physical mappings while leaving direct intents available.

`Staged` means the candidate takes effect when the callback successfully
settles. The last valid replacement in that callback wins. `InvalidMappings`
leaves the current mapping set and any earlier valid candidate unchanged;
callback failure discards the staged replacement. `Unavailable` reports a call
outside those supported callbacks (such as Attach, Shutdown, debug or timeline
completion). Duplicate mapping IDs, unknown intents, incompatible value kinds,
and unsupported controls are invalid. Distinct mapping IDs may deliberately share a physical trigger.

At runtime, a successful replacement uses the lifecycle transition or advances
the input control revision and clears held and pending input through the
existing input lane. During creation it instead selects the initial map before
the lane admits input. Old bindings stop firing, and queued events from the previous binding cannot trigger stale actions.
Products receive the normal clear fact and must release their derived held
state. Focus and text-entry suppression continue through the same lane.
`ProductCreateContext.Input` remains the initial composition snapshot; products
own their chosen settings, UI and persistence.

### Gameplay cursor mode

Keyboard-driven products without mouselook can opt into a free cursor:

```xml
<RustyEngineProductInputCursorMode>unlocked</RustyEngineProductInputCursorMode>
```

The default is `pointer-lock`, preserving FPS behavior. In `unlocked` mode,
clicking the canvas focuses gameplay keyboard input without requesting pointer
lock; pointer movement does not supply camera-look deltas. Marked DOM controls
remain usable, and Engine input still owns focus loss, clearing, and rebinding.
The setting is available as `context.Input.CursorMode` and is carried through
the packaged Product and browser bootstrap. Invalid values reject staging.

### Default browser lighting

The packaged browser shell keeps its neutral world and viewmodel light rigs by
default. A product can disable either rig independently through ordinary build
properties; retained lights created through `Graphics` continue to be realized.

```xml
<PropertyGroup>
  <RustyEngineProductDefaultWorldLights>disabled</RustyEngineProductDefaultWorldLights>
  <RustyEngineProductDefaultViewmodelLights>neutral</RustyEngineProductDefaultViewmodelLights>
</PropertyGroup>
```

Each value is `neutral` or `disabled`. These are host defaults, not product
lights: disabling the world rig does not change the viewmodel setting or remove
product-owned point, directional, or spot lights. Invalid values reject staging.

## Read bundled product files

Set `RustyEngineProductContentRoot` to the authored or build-generated content
directory. Generate build-time files before `StageRustyEngineCoreClrProduct`
runs. The SDK stages that tree under the Product bundle's `content/`; the host
resolves the manifest location and supplies `ProductCreateContext.Content`.
Game code does not need executable-relative paths, working directories or URLs.

```csharp
ProductContent content = context.Content;
var rules = JsonSerializer.Deserialize<CombatRules>(
    content.ReadBytes("rules/combat.json").Span);

foreach (ProductContentFile file in content.ReadDirectory("rules/enemies"))
{
    // RelativePath is content-root-relative; Name is the final filename.
    // Deserialize with the product's own types/options or source-gen context.
    var enemy = JsonSerializer.Deserialize<EnemyDefinition>(file.Bytes.Span);
    enemies.Add(enemy.Id, enemy);
}
```

`ReadFile` returns path and bytes; `TryReadFile` handles optional files without
an exception. `ReadBytes` returns admitted memory and `ReadText` decodes UTF-8
(including an optional UTF-8 BOM). Required reads throw `FileNotFoundException`
with the missing logical path. Names are case-sensitive and use `/` separators.

`ReadDirectory` returns an array sorted by full relative path using ordinal
comparison. It reads immediate children by default; pass `recursive: true`
for descendants. `""` selects the root; a trailing slash is optional. An absent
or empty directory returns an empty array. Files such as `_index.json` have no
special Engine meaning: a product may find that conventional filename, read
authored ordering/IDs and build its own dictionary without embedding each
definition's filename in code. Do not depend on an index occupying element zero.

The existing `Files`, UTF-8 `Path` and `Bytes` members remain available. Content
is an eagerly admitted memory snapshot copied across the generated boundary;
these helpers add no filesystem reads, streaming, parsing framework or writable
store. Treat retained path/payload memory as read-only. `rusty dev` restaging
supplies a new snapshot on product replacement. Tauri and sealed-container host
flavors remain packaging investigations; these logical names do not promise an
implemented standalone browser/WASM or Tauri runtime.

### Independently loaded content bundles

Declare groups beneath `RustyEngineProductContentRoot` in the ordinary product
project; omit `Root` when it equals the bundle ID:

```xml
<ItemGroup>
  <RustyEngineContentBundle Include="procgen" />
  <RustyEngineContentBundle Include="ui-art" Root="media/ui" />
</ItemGroup>
```

SDK staging recursively inventories each declared directory, recording file
names, lengths and SHA-256 identities in Engine-owned `.rusty-bundles.json`.
Do not author that file. Restaging regenerates it after edits, additions or
removals; JSON and asset bytes retain their formats. Bundle IDs are ASCII
letters/digits followed by letters/digits, `.`, `_` or `-`. Roots are relative,
nonempty directories; duplicate IDs and overlapping roots are rejected.

```csharp
ReadOnlyMemory<ContentBundleInfo> available = context.Content.ListBundles(); // metadata only
using ProductContentBundle bundle = context.Content.OpenBundle("procgen");
foreach (ContentReferenceInfo entry in bundle.Entries.Span) { /* metadata only */ }
var index = bundle.ReadText("_index.json");
ProductContentFile[] definitions = bundle.ReadDirectory("rooms", recursive: true);
// Native content consumers can avoid a managed byte copy:
using ContentReference source = bundle.OpenReference("rooms/entrance.json");
```

Declared bundle files are excluded from the legacy `ProductContent.Files`
snapshot, global named reads and browser initial-content payload. Discovery
reads only the inventory. Opening a bundle reads and verifies that collection's
files into an immutable Rust snapshot; it does not load other bundles or copy
all its bodies into C#. `Entries` exposes copied metadata; `ReadFile`, `ReadBytes`,
`ReadText` and `ReadDirectory` copy the requested bodies. A read borrows the Rust
source through a range lease and copies it once into managed storage, with no
intermediate chunk buffers. Bundle/file inventories already arrive in Engine UTF-8 path
order; helpers do not sort them again or list every bundle before an open.
Directory semantics
match ProductContent, with **bundle-relative** paths; bundle ordering follows UTF-8 path order.
Each open has independent ownership; a second open is not a global cached mount.

Disposing a bundle releases its collection ownership and prevents further
helper reads. Previously returned managed bytes remain valid. An independently
opened `ContentReference`, admitted authored catalog, or created Engine resource
has its own lifetime and must be disposed separately; bundle closure does not
cascade-delete resources or invalidate consumers. Retained reference identity
uses the original content-root-relative path and hash; `ResolveReference` can
resolve it while its owning bundle is open. Cross-bundle dependencies are
explicit product composition: open the required bundles and pass their content
references to the typed services. A reference also retains its source
collection's immutable dependency context: GLB companion images/buffers resolve
relative to that GLB inside the same bundle. Opening an unrelated bundle cannot
change resolution. After admission, the Engine resource retains its own payload;
it no longer needs the source reference or bundle. Appearance keeps imported
results for the current service lifetime so repeated opens of the same admitted
source avoid decoding, packing and hashing again. Reuse checks immutable buffer
identity, including each GLB dependency. A newly admitted source snapshot is
imported afresh; service reload discards these derived results. Resource handles
still acquire and release their own per-open ownership.

### Live content from product-owned sources

An editor can read a mutable file or generated bytes after startup and admit an
immutable snapshot without rebuilding the product bundle:

```csharp
using ContentReference source = engine.Content.AdmitReference(
    new ContentAdmissionRequest("preview/model.glb", modelBytes,
        new ContentSourceFile[] { new("preview/texture.png", textureBytes) }));
RenderResource model = engine.Animation.OpenAnimatedMeshFromContent(
    new AnimationContentRequest(source));
Appearance appearance = engine.Animation.CreateAnimatedMeshAppearance(
    new AnimatedMeshAppearanceRequest(model));
```

The product owns file selection and reading. Paths are logical, relative names,
not filesystem access instructions. Dependency names are in the same logical
root as the source; GLB URIs resolve relative to its directory. Use an empty
dependency array for a self-contained GLB. Admission copies the source and
companions once into Engine ownership and never changes the startup catalog or
other references. `ResolveReference` does not reopen a transient snapshot;
re-admit current bytes when restoring an editor selection.

The existing animation renderer preserves materials, textures, skins and clips;
`Animation.ReadMeshInfo(resource)` exposes admitted bounds and material/joint/clip
counts; `ReadClips(resource)` copies each
clip ID, name and duration. Use its ordinary instance/playback APIs for animation.
Dispose instances, publish the snapshot without their appearances, then dispose
appearances and resources. Direct-instance teardown also accepts an already
published removal snapshot and avoids sending a stop to that retired target. The source reference can be disposed immediately
after resource admission. Transient animation imports do not enter the
startup-source import cache, so closing the reference and resource releases
these snapshots. A malformed or incomplete GLB throws `EngineCallException`
with operation diagnostics without failing the surrounding product callback.
Admit the replacement before releasing the previous selection to keep it visible
when an import fails.

Use the same asset admissions during Create or a later product update:

| Asset | Bundle consumer | Existing format |
| --- | --- | --- |
| Images and textures | `Graphics.OpenResourceFromContent` | RGBA PNG |
| Packed static geometry | `Graphics.OpenResourceFromContent` | `.rmesh` |
| Authored static mesh | `Graphics.CreateStaticMeshFromContentReference` | StaticMeshAsset JSON with inline payload |
| Animated meshes and animation packs | `Animation.OpenAnimatedMeshFromContent`, `OpenAnimationClipPackFromContent` | GLB, including same-bundle relative dependencies |
| Fonts | `Graphics.OpenResourceFromContent` | WOFF2 |
| Audio clips | `Audio.OpenClipFromContent` | WAV |
| Full-viewport video | `Video.Play`, `Video.Stop`, `Video.Skip` | WebM (`video/webm`; VP9+Opus or video-only VP9) |
| Voxel assets, objects and annotations | `VoxelContent.LoadAssetFromContent`, `LoadObjectFromContent`, `LoadAnnotationFromContent` | Existing typed JSON formats |
| Imported voxel models | `VoxelContent.LoadMagicaVoxelFromContent` | MagicaVoxel `.vox` |
| Authored catalogs/prefabs/scenes and spatial artifacts | Existing typed ContentReference consumers | Their existing Engine document formats |
| Text and arbitrary bytes | Bundle `ReadText`, `ReadBytes`, `ReadDirectory`, or Content reference reads | No asset decoder required |

The bundle is a source container; an admission still applies the relevant
Engine format rules. It does not make arbitrary image, model or audio formats
supported. The Engine supplies admitted resource bytes to its renderer/audio/video
host, including assets first loaded after startup and fresh client attachments.
Products do not extract bundle files or build renderer URLs.

Missing bundles/files report their logical names. A bundle whose files no
longer match its staged inventory fails to open; rebuild/restage it. This is a
directory-backed build-content capability for the current CoreCLR/NativeAOT
hosts. It adds neither archive extraction nor a standalone browser runtime, and
does not promise that closing a collection frees independent GPU resources or
forces managed garbage collection. Browser DOM image delivery remains a
separate consumer concern; bundle discovery alone does not produce image URLs.

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

For explicit staging without launching, run
`dotnet msbuild /path/to/Example.Game.csproj -t:StageRustyEngineCoreClrProduct -p:Configuration=Release`.
A plain `dotnet build` compiles the project but does not request this staging
target. Use the target (or `rusty dev`) to regenerate `obj/Rusty.Engine/Product`.

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

### Controller input in product menus

With selected-controller input enabled, `mountUi` receives an optional
`context.input.subscribe(observer)` port. The existing host controller cadence
delivers immutable `{ context: 'interface', fact }` observations while
`context.ui.setInteractionMode('interface')` owns input. Facts use the Engine's
normalized `controller-button` pressed/released edges, `controller-axis`
samples, and `controller-button-value` pressure changes. UI owns their menu
meaning; these are not Rust-mapped gameplay intents. Unsubscribe on UI disposal.

Interface observations never enter the gameplay queue or consume its sequence
numbers. Use `context.intents.claim` for actions that need product processing;
it remains the sole ordered command lane. No downstream gamepad polling or
animation loop is needed. Gameplay mode keeps ordinary Engine input delivery;
modal mode and loss of browser focus suppress controller observation. Mode
changes adopt already-held controller state without replaying its press, so a
menu-opening button does not immediately activate or close the new menu.

### Host exercise contract

`rusty-product-host --exercise` runs the Engine's provider fixture assertions,
not a general product health check. It expects fixture-specific behavior,
including create-time and Start UI projections, a retained voxel baseline,
configured exercise input, acceptance of timeline ticket `7`, and deliberate
fault/restart behavior. See `fixtures/csharp-nativeaot-trial/Product.cs` for
the fixture used by the Engine's package checks. Ordinary products do not need
to implement these assertions.

The fresh-attachment assertion requires `defineMaterial`, `create`, and
`replaceMeshPayload` together in one retained frame, then checks that a second
attachment preserves the baseline and runtime readout. Its failure reports
observed operation kinds and missing kinds separately for each frame (or that
no frame was published). This is a fixture expectation, not a universal shape
for valid graphics.

The fixture uses the existing safe services: create a `Spatial` session, apply
nonempty `Voxel` edits, bind the used material slots, and retain a
`VoxelScenePresentation.ProjectSceneDirectional` projection. These facts must
be committed during construction/Start before attachment; call `RefreshScene`
after subsequent edits. The Engine builds and retains the renderer operations.
Product metadata alone does not create voxel geometry, and initialization in
`Attach` is not invoked on browser connection.

A product with static meshes, sprites, or no mesh content should not add dummy
voxels or fixture callbacks to pass this check. Launch normally with
`rusty dev --project <product.csproj>`, or run the matched host with
`rusty-product-host --product <staged-Product-directory> --loader coreclr`
without `--exercise`. Verify the product's actual startup and interactions
through that host; fixture success is not evidence of gameplay correctness.

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
`EntityGraphicsProjection` also accepts an optional parent `EntityId`.

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
`SetBackgroundColor(new(new Color(r, g, b, 1)))` selects an opaque retained
viewport clear color and replaces any selected sky. `SetSkyBackground` replaces
that color with a retained panorama; `ClearSkyBackground` returns to the Engine
default. Products choose the color or resource while the Engine owns renderer
state and realization.

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

## Offline images and GLB export

`engine.RenderOutput` owns asynchronous output from the current retained
appearance snapshot. `CaptureImage` and `ExportSceneGlb` select a product
`AppearanceFact.ObjectId`, including its descendants and ancestor transforms.
They freeze the scene at successful callback completion; later product changes
cannot alter that job. A missing source/camera or unsupported export feature
produces a failed job with a UTF-8 diagnostic, rather than partial success.

```csharp
RenderOutput image = engine.RenderOutput.CaptureImage(new(
    sourceObjectId, camera, 512, 512, new Color(0, 0, 0, 0), false,
    1, CaptureToneMapping.AcesFilmic, 4, animatedObjectId, "run", .5));
RenderOutput glb = engine.RenderOutput.ExportSceneGlb(new(sourceObjectId, true));
// During later product callbacks:
if (engine.RenderOutput.Read(image).State == RenderOutputState.Completed)
{
    ReadOnlyMemory<byte> png = engine.RenderOutput.ReadBytes(image);
    // Product chooses a file, store, or other destination for these copied bytes.
    image.Dispose();
}
```

Image dimensions are independent of the window. The result is a top-to-bottom
8-bit sRGB RGBA PNG with straight alpha. Clear colors use linear RGB;
`UseCameraBackground` instead selects the current `CameraView` sky/color.
Existing retained lights, material assignments, camera framing and projection
remain Engine inputs. Exposure, no tone mapping/ACES, and multisample count are
explicit capture choices. Unsupported target dimensions or sample counts fail
with a diagnostic. A zero `PoseObjectId` keeps the frozen pose; a nonzero object
selects an exact normalized clip time in `[0,1]`, including the final pose,
without advancing the live animation or wall clock.

Completion means resources loaded, pose evaluated, render/readback finished,
and PNG/GLB bytes copied to the Engine owner. Poll `Read`; use `ReadDiagnostic`
for a failed job. `Cancel` or `Dispose` prevents later completion from reviving
a job. Dispose results after use; the renderer reuses its batch render target.
Requests settle after a callback, so never block that callback waiting for one.

GLB exports current retained geometry, hierarchy, transforms, standard material
and texture assignments, normals and UVs, rather than returning the original
imported file. `IncludeAnimations` preserves supported skin/clip data. Unsupported
shader-based materials or animation features fail explicitly. Generated meshes
remain exportable after the implicit field has been disposed, as long as the
mesh appearance is retained when the request settles. Reopen output through the
`Animation.OpenAnimatedMesh` / `CreateAnimatedMeshAppearance` content path
(which also admits static GLBs with no embedded clips).

For unattended batches, launch the packaged `rusty dev --headless` or
`rusty-product-host --headless`. Chromium must be installed; `RUSTY_CHROMIUM_PATH`
selects its executable. This uses the Engine browser backend in a managed
headless process, including software WebGL support; it is not a GPU-free
renderer or a product-owned DOM screenshot path.

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
| [`Rusty.Engine.Entities`](../csharp/Rusty.Engine/Entities) | Ordinary class/value component storage, scoped value edits, and managed adapters around Engine mechanisms, compiled into `Rusty.Engine`. |
| [`Rusty.Engine.Persistence`](../csharp/Rusty.Engine/Persistence) | Explicit product-state codecs and stores for the current shape, compiled into `Rusty.Engine`. No product schema versions or migrations: breaking the shape breaks old saves by product choice. |

Use a helper when it fits the product's real domain. A product may compose its
own ordinary C# architecture instead. None of these packages implies a hidden
`ProductApplication`, `ProductBuilder`, `IProductModule`, analyzer suite,
typed-content framework, or projection framework: those names are not current
SDK APIs.

Product- and Kit-owned typed services with meaningful rule-resolution extension points
are ordinary C# composition, not Engine framework extension: the Engine ships no gameplay
bus, plugin registry, or RPG rules, and Read → Decide → Apply → Publish remains a
product-style option (see [C# product style](csharp-product-style.md)), never an Engine
protocol.

### Entity stores, mechanics stores and Engine adapters

`EntityStore` holds managed entity/component facts; `InventoryStore` holds the
inventory, item and equipment ledger. Their identities and revisions are local
to their owning stores. The entity adapters read those facts and call named
Engine mechanisms; they do not create another entity world or own native
resources supplied by the caller.

The completed naming migration is source-breaking (the left column is historical):

| Previous name | Current name / responsibility |
| --- | --- |
| `EntityWorld` | `EntityStore` — managed entity/component storage |
| `InventoryWorld` | `InventoryStore` — inventory/item/equipment storage |
| `InventoryWorldCandidate` | `InventoryEdit` — detached inventory edit |
| `AppearanceEntityWorld` | `EntityGraphicsProjection` |
| `SpatialEntityWorld` | `EntityTriggerProjection` |
| `MotionEntityWorld` | `EntityMotionResolver` |
| `KinematicEntityWorld` | `EntityKinematicMotion` |
| `CharacterEntityWorld` | `EntityCharacterController` |
| `DynamicsEntityWorld` | `EntityDynamicsAdapter` |
| `WorldOriginEntityWorld` | `EntityOriginRebaser` |
| `EntityWorldDebug*` | `EntityStoreDebug*` — module, selector, projection and debug snapshot types |
| `EntityWorldDiagnostics` | `EntityStoreDiagnostics` |
| `PhysicsWorld` (C# configuration) | `PhysicsSettings` |

Related adapter guards/results follow their owning adapter name and use
`StoreRevision`. `InventoryView.StoreRevision` identifies the whole inventory
store revision; its existing `InventoryRevision` identifies the individual owner
inventory revision. Item receipts use `InventoryRevisionBefore` and
`InventoryRevisionAfter`. Debug registration uses
`RegisterStore`, `ReplaceStore` and `UnregisterStore`; `entity.stores` lists
registrations and debug output identifies them with `store=` / `stores=`.

`DynamicsWorld` remains a disposable native simulation owner. `WorldOrigin`
continues to mean the spatial coordinate origin. Neither is a managed entity
store. The Rust spatial implementation's internal physics type is unchanged.

`EntityStore` now accepts ordinary classes and value components in the same
store. Unused whole-store snapshot/restore, callback mutation batches, component
copy codecs and entity-persistence wrappers have been retired.
Use one exact SDK/runtime pair per [distribution](csharp-distribution.md);
do not combine a renamed SDK with a previously published runtime merely
because layouts look similar.

### Ordinary component attachment

```csharp
using var entities = new EntityStore();
EntityId actor = entities.Create();
var health = new Health { Current = 10 };
entities.Add(actor, health);
entities.Get<Health>(actor).Current -= 2;
// health.Current is now 8: this is the same attached object.
foreach (var row in entities.Query<Health>())
    Console.WriteLine($"{row.Entity.Value}: {row.Value.Current}");

sealed class Health
{
    public int Current { get; set; }
}
```

No component base class, interface, numeric key, registration or codec is required.
The explicit generic `T` selects one family per entity: `Add` rejects an occupied
slot, `Replace` requires an existing slot, and `Remove<T>` returns false when absent.
Null components are rejected. `Has<T>` and `TryGet<T>` inspect membership without
registering a family. An interface/base family is available by explicitly choosing
that generic type; the store does not scan an object's inheritance hierarchy.

`Get` and query rows return the actual class instance. The normal C# aliasing rules
apply: components usually have one semantic owner, but deliberate sharing is
allowed. Removal, replacement, entity destruction and store disposal release
attachments without invalidating references already held by product code and
without disposing the component or its native resources. Product owners perform
any necessary cleanup. Store IDs are local to one store lifetime.

Queries capture membership immediately, in increasing entity-ID order. Adding,
removing, replacing or destroying attachments afterward does not change that
returned list. Class objects within the list remain live references, including
objects subsequently detached from the store. Disabled entities are excluded
unless `includeDisabled: true`; two-family `Query<TFirst, TSecond>` joins use the
same rules. Use the store at the product's normal execution boundary, not as a
concurrent object database.

Useful value facts remain structs. `Set(entity, value)` explicitly inserts or
replaces a struct; `Add`/`Replace` also work for them. Value reads are ordinary C#
copies, so nested references are shared unless the product explicitly copies
them. Prefer a class for mutable reference-bearing state. Existing registered
`ComponentType<T>` descriptors and generic access address the same family, not
parallel storage. A descriptor can be registered after generic attachment; a
second explicit descriptor for that same `T` is rejected. Descriptor keys used
by diagnostics for automatically attached families are store-local implementation
details, not durable serialization identities.

Versions describe explicit attachment, removal, replacement and lifecycle
changes. They do not observe fields or methods on a returned object, and replacing
a class with the same instance is a no-op. Destroy releases entity/component rows
and containment edges; it detaches children without destroying them. IDs stay
nonzero and monotonic, with no reuse.

### Entity metadata and the optional Actor facade

`EntityStore.Create` accepts an `EntityTypeId`: kind/origin metadata describing what an
entity is, independent of its unique runtime `EntityId` and any product-owned durable
identity. The value is free-form and fixed at creation — `"code:spawn/goblin-scout"` needs
no authored definition or registry — and defaults to `EntityTypeId.Unspecified`. Metadata
travels with the canonical record and shows in `entity.list` / `entity.get` debug output.
Read it with `GetTypeId`; it is not a component.

`Actor` is an optional sealed facade over one existing entity for discoverable typed access:

```csharp
var actor = new Actor(entities, hero);
StatsComponent stats = actor.Get<StatsComponent>();
EntityTypeId kind = actor.TypeId;
```

Every member reads the store live, so named properties return the same attached instances —
never copies or a mirrored state graph. Wrapping attaches nothing: unknown entities throw,
missing components throw the store's ordinary `InvalidOperationException`, and releasing the
facade never affects the entity (there is nothing to dispose). Downstream Kit and ruleset
actors compose their own small wrappers holding an `Actor` rather than inheriting from it.

### Explicit edits and persistence

Ordinary access never deep-copies components. Direct gameplay methods need no
transaction session or receipt graph. The unused `Rusty.Engine.Resolution`
module has been removed; optional Application and StateMachine helpers remain.

For callers that need prepared value replacements, `EntityBatch.Set` and
`EntityBatch.Create` describe closed operations. `EntityStore.PrepareBatch`
returns a disposable `EntityEdit`. Preparation copies index maps and only the
value families it changes; unrelated attached classes retain their identity.
Publication checks the store's structural revision. It does not freeze class
internals or roll back nested references or external owners. A failed or disposed
edit cannot publish; successful publication is idempotent. Receipts report
structural revisions, not an ambiguous mutation count.

D20 uses this path for value facts. Entity/native adapters capture their selected
values before native commits and publish typed replacements afterward. Their native
lifetime and failure rules remain in force. No arbitrary mutation callback or
whole-store restore is offered.

`InventoryEdit` retains detached planning required by inventory operations.
Failed operations, stale publication, cancellation and disposal close the edit;
none can publish earlier partially staged operations afterward.

Explicit saves use `ProductStateStore<T>` with a product-defined codec and data.
The product decides what to capture, validates/rebuilds a candidate when needed,
and adopts it. Bounded debug snapshots remain observations, not live state owners.

## Ordinary numeric stats

`Rusty.Engine.Mechanics.Stat` is one mutable, double-backed object for whole-number
and fractional gameplay values. It works independently or inside a class component.

```csharp
var strength = new Stat(40, minimum: 0, maximum: 100);
var equipment = strength.AddModifier(4);
strength.AddModifier(1.5, StatModifierKind.Multiply);
int attackStrength = strength.ValueInt; // (40 + 4) * 1.5 = 66
strength.RemoveModifier(equipment);
strength.BaseValue = 42;

var speed = new Stat(1, minimum: 0, maximum: 1);
var slow = speed.AddModifier(0.4, StatModifierKind.Maximum);
float movementFactor = speed.ValueFloat;
speed.RemoveModifier(slow);
```

`Value` is a double. `ValueFloat`, `ValueInt` and `ValueInt64` are explicit
conversions; out-of-range conversions throw instead of wrapping or producing
infinity. Integer getters default to nearest with midpoint away from zero.
Set `integerRounding: MidpointRounding.ToZero` for rules that truncate; this does
not round the underlying value. There are no implicit numeric casts.

Base values and contributions must be finite. Default bounds cover finite double
values, with no fixed gameplay ceiling. `Minimum`/`Maximum` or `SetBounds` change
bounds. `quantum: 0.25` rounds evaluations to quarter units; zero (the default)
disables this. `rounding` selects the evaluation rounding mode separately from
integer conversion. Selected additions precede multipliers, then quantization
and the final clamp to resolved bounds. Off-grid endpoints win: a rounded 0.5
with maximum 0.4 produces 0.4. Invalid arithmetic, bounds or modifier changes
leave the previous valid stat and modifiers unchanged.

For authored provenance, `SetSources(statId, sources)` accepts `StatSource` values
with typed contributions, priorities and stacking groups. Sources sort by priority,
identity and definition; equal-strength selections keep the first in that order.
`UniqueByDefinition` selects the first activation of each source definition.
`RemoveSource` removes one activation; `Explain()` returns an immutable evaluation
readout with applied, suppressed and inapplicable decisions. These are optional;
ordinary `AddModifier` needs no source identities or operation records. Local
modifiers keep insertion order and precede authored contributions within each
operation phase. Modifier handles are local to their creating stat.

Inventory quantities, capacity and entity identities remain checked integers and
do not pass through floating-point stats. The old Exact/Continuous stat and track
families have been retired.

## Resource tracks

A `Track` references its actual maximum `Stat` and owns its current value:

```csharp
var maximum = new Stat(100, minimum: 0);
var health = new Track(maximum, current: 70);
health.Spend(10);
maximum.BaseValue = 120; // health is now 60/120
health.Restore(200);    // clamps to 120
bool paid = health.TrySpend(130); // false, current stays 120
```

The fixed-maximum convenience `new Track(100)` creates a Stat internally. Current
defaults to the maximum; minimum defaults to zero. `Maximum` returns the same Stat
object, `MaximumValue` reads its value, and `Current`/`Value` read the track value.
`ValueFloat`, `ValueInt` and `ValueInt64` provide the same checked conversions as
Stat. Spending/restoring rejects negative or nonfinite amounts. `Spend` throws on
insufficient value; `TrySpend` returns false without mutation. `Restore` saturates.
Spend/Restore return the actual applied amount. `SetCurrent` rejects out-of-bounds
values unless explicitly called with `clamp: true`.

By default, maximum changes preserve current and clamp it to the new bounds.
`maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount` instead
preserves the missing amount: 70/100 becomes 90/120; 90/100 becomes 70/80.
All tracks sharing that Stat reconcile synchronously before a Stat mutation
returns. If a dependent track would have maximum below minimum, the entire Stat
change is rejected before any dependent changes. Dependencies are weak references;
abandoned tracks do not keep imposing their minimum or require disposal.

Track quantization/rounding and integer conversion are constructor policies,
independent of its maximum's policies. Use `quantum: 1` and explicit rounding for
whole-number rules. Endpoints remain reachable even off the grid. Changing Minimum
validates first and clamps current as needed. Direct operations need no revision,
receipt object, candidate or publish step.

For an actual preview, `Stat.Copy()` creates independent numeric state without
copying dependent tracks. A product can construct a new Track around that copy,
validate its grouped plan and adopt its chosen state. D20 uses this for action
planning and Dagger for level-up preflight; this is explicit product orchestration,
not a promise of automatic graph rollback. Copies receive independent local
modifier handles; authored source identities remain available for source removal.

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
  `ImplicitTextureMapping` optionally replaces that scalar chart: use
  `MajorAxis(scale, offset)` for independent U/V repeats and offsets, or
  `Basis(uAxis, vAxis, scale, offset)` for an orthonormal U/V orientation in
  the field's extraction coordinates. Its scale is repeats per projected world
  unit and its offset is added after scaling. `RecipeSurface.Placement` is
  applied after extraction and does not reproject UVs. Leaving the mapping at
  its default preserves the legacy major-axis UV output exactly.
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

For streaming authored collision cells, use `Spatial.ApplyCollisionResidency`
with stable asset and instance IDs. Its arrays are upserts; `RemovedAssets` and
`RemovedInstances` remove selected IDs before upserts are applied. Missing
removals are harmless. A whole delta commits atomically, and removing an asset
still referenced by a retained instance fails without changing the scene.
Unchanged geometry and prepared colliders are shared; admission does not copy
or rebuild every resident cell. Use current local-frame instance transforms;
`WorldOrigin` rebases these same retained colliders. The product owns cell
selection, unload/reload policy and its authored identity map. This path uses
ordinary static-mesh collision/query ownership, without a dense voxel volume
or replacement of the complete collision artifact.

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

`PlanarRecipes.ConvexPrism` accepts either winding of a finite, strictly convex
XZ contour. It rejects concavity, self-intersections and degenerate edges before
adding field nodes. `PlanarRecipes.Walkway` unions square-capped segments and
preflights the whole centerline before construction. These are composition
helpers over the existing Engine field operations, not another evaluator.

`RoomRecipes.Shell` emits floor, wall and ceiling solids from one interior box,
wall thickness and named portal boxes. Optional floor platforms and ceiling
soffits union into their owning slabs. The returned `RecipeRoomContinuity`
contains expected wall/slab contacts (excluding doorway intervals), the same
portal boxes and an interior seed. Resolve its named `RecipeJoin` surfaces to
captured audit IDs, then use `Request` with the existing `ReadExpectedJoin` or
`ReadEnclosure` services. Products choose budgets and interpret completeness;
declarations express intent, not a guarantee of a clean extracted mesh.

The optional `RoomRecipes.Shell` placement transforms emitted surfaces and their
declarations together. Individual joins support placements preserving a rectangular
contact patch; room enclosure caps require axis-preserving placement
because the underlying cap contract uses axis-aligned boxes. Arbitrary rotated
caps are rejected rather than silently enlarged. Mesh generation and audit
capture must still happen synchronously before a recipe field is disposed.

The [architectural room example](../fixtures/csharp-architectural-room/README.md)
shows a recessed floor, stepped ceiling, windows, door and adjoining passage
through the ordinary packaged C# path. The source measurements → editable
suggestions → manual composition workflow in Loading Bay is a useful authoring
pattern; these helpers do not import source levels or prescribe their layout.

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



An interrupted development-host output subscription reattaches through a fresh
retained baseline, even after receiving numbered output. SSE cursors are local
to a host process; they are never reused after interruption against a potentially
replaced process. Input and product mutations are not replayed during this
recovery. The browser remains gated until the replacement projection is applied.

### Opt-in authored surface audit

`ImplicitSurfaces.CreateAudit()` creates an authoring-only collection. In a
`RecipeWriter` receiver, generate the ordinary mesh, then call
`CaptureAuditPiece(new(audit, pieceId, surface.Field, surface.Root, mesh,
surface.Placement, service.ReadGeneration(surface.Field).SampleSpacing))`.
Use stable unique `ulong` piece IDs and keep a product dictionary for labels.
The Engine copies geometry and retains an independent field snapshot; source
fields, meshes and materials can be disposed before running the audit.

`ReadAudit(new(audit, toleranceCells))` returns copied diagnostics, candidate
piece-pair and triangle-pair counts. Each diagnostic identifies both pieces,
classification, world-space bounds and approximate affected area. Coincident
and near-coincident exposed surfaces are distinct from buried surfaces; an
ordinary solid intersection need not be an exposed conflict. The report does
not change meshes or decide product acceptance. Dispose the collection when
authoring analysis ends; managed reports survive its disposal. Capture and
analysis are explicit synchronous operations, with no ordinary update cost.

The audit compares nearly parallel extracted facets and clips their projected
triangles to estimate contact area, with spatial AABB filtering for piece and
triangle candidates. It uses field **signs**, rather than treating constructive
field values as distances. Tolerance is relative to the coarser world-space
extraction spacing of each pair (largest placement stretch for nonuniform
scale). Facet normals must be within about 2.6 degrees of parallel; separation
within coordinate floating-point resolution is classified as coincident,
and the remainder up to that tolerance as near-coincident. Side probes start
at representable coordinate resolution and grow only when needed to straddle
an extracted facet. Exposure and burial sample patches at extraction spacing,
so pairwise area totals can count opposing internal faces separately.
This is sampled diagnostic evidence: curved
DC approximation, thin features or gaps between samples, open/clipped extraction
boundaries, and partial buried-face coverage can make bounds/areas approximate
or contacts unreported. A clean report is not a mesh-validity certificate.
Camera depth precision, extreme near/far ratios, shadows, transparency sorting,
texture aliasing and shader artifacts are outside this audit's guarantees.

The same collection supports three separate continuity queries:

- `ReadMeshIntegrity(new(audit, openings))` examines every captured triangle's
  exact-position connectivity. It reports open edges, edges with more than two
  incident faces, disconnected vertex fans and zero-area triangles. Duplicate
  vertex positions share connectivity; nearby positions are never welded.
  `ImplicitAuditOpenRegion` declares a world-space box for one stable piece ID;
  an edge is intentional only when both endpoints lie inside it. Use this first
  on malformed meshes; the overlap analysis requires nondegenerate facets.
- `ReadExpectedJoin(new(audit, pieceA, pieceB, center, halfU, halfV,
  searchDistance, toleranceCells, sampleSpacing, maxSamples))` samples an authored
  rectangular contact patch. Its perpendicular half axes set orientation and
  size. Along the patch normal, the query finds each named mesh's nearest
  intersection within the search radius. It reports separation above the
  extraction-relative tolerance, or `MissingJoinSurface` when either side is
  absent. Choose the patch and radius to identify the intended surfaces, avoiding
  unrelated faces of the same pieces. Width is maximum sampled separation;
  affected area is the sum of failed patch cells. Closed-mesh ray containment
  distinguishes solid overlaps from air gaps using extracted geometry, even
  when the source fields still meet. Inconsistent containment rays produce
  incomplete coverage. Open meshes provide facet-separation evidence without
  a closed-solid containment guarantee. This is not inferred intent.
- `ReadEnclosure(new(audit, minimum, maximum, interior, openings,
  sampleSpacing, maxSamples))` searches a bounded six-neighbor world grid from
  the declared interior point. Segment/triangle intersections block traversal.
  A leak returns one interior-to-outside `Path` and exit bounds; it does not
  enumerate every leak. Declare intentional door/window volumes as
  `ImplicitEnclosureOpening` entries: these virtually cap those openings while
  searching for other routes. `IntentionalOpening` means a declared virtual cap
  was encountered, not that a physical door was proved open. Capture moving
  doors at the pose being audited;
  use a separate collection for another pose. The query examines mesh barriers,
  not field distances or an assumed union of closed solids.

Continuity reports copy diagnostics and path points before releasing their
native lease. `Complete != 0` means the declared discrete query completed (or
found a witness); it is not proof below `Resolution`. For joins/enclosures,
`Sampled` counts patch samples/visited cells. An insufficient `maxSamples`
budget returns `IncompleteCoverage` and `Complete == 0`, never a clean result.
An enclosure seed whose connection to its grid cell crosses geometry also
returns incomplete coverage. Seeds must be authored in empty interior space.
Thin passages, diagonal connectivity, small missing triangles and contacts
between samples can be missed. Repeat at finer spacing when a feature is near
resolution; exact topology findings and sampled enclosure findings are distinct.
A leak path's reported width is the grid spacing, not measured clearance.
Enclosure diagnostics use zero piece IDs for collection-wide connectivity;
the declared region and returned path identify their scope.

These are synchronous authoring operations. For full-scene capture and analysis
that exceeds development worker deadlines, launch the packaged product with
`rusty dev --project <product.csproj> --debugger` before requesting the audit.
That supported lane disables worker startup/callback deadlines for the session;
it does not make analysis asynchronous or increase geometric coverage. Keep
analysis behind an explicit authoring switch or debug command, and stop the
owned host after the report is collected.

### Entity stats collections

`StatsComponent` holds product-named `StatId → Stat` and `TrackId → Track`
collections. Use it standalone or attach it like any ordinary class:

```csharp
var maximumId = StatId.Parse("maximum-health");
var healthId = TrackId.Parse("health");
var maximum = new Stat(100, minimum: 0);
var stats = new StatsComponent();
stats.AddStat(maximumId, maximum);
stats.AddTrack(healthId, new Track(maximum));
entities.Add(entity, stats);
entities.Get<StatsComponent>(entity).GetTrack(healthId).Spend(10);
foreach (var (id, track) in stats.Tracks) { /* UI reads track.ValueInt */ }
```

`Stats` and `Tracks` are live read-only dictionary views; their objects remain
mutable. Add methods reject duplicate IDs and retain the exact supplied objects.
`GetStat`/`GetTrack`, `TryGetStat`/`TryGetTrack`, and removal methods operate on
those collections directly. A track's maximum is registered only if the product
explicitly adds it. Removing a stat entry does not disconnect tracks referencing
that stat. IDs, labels, formulas, save schemas, and gameplay policy stay product-owned.

### Effects and owner-scoped inventory components

`EffectsComponent` is an ordinary class for Apply/Refresh/Replace/Remove/Expire
operations, with stacking and provenance checks. It works standalone or attached
through `entities.Add(entity, effects)`. Product code owns duration and timing.
Use `effects.Copy()` only when a detached preview is useful: the copy has its own
collection and shares immutable effect entries/definitions, without replaying
mutations. D20 uses this for action planning; Rifles keeps its own effect clock.

`InventoryStore` owns item quantities, containment and equipment records.
Use its direct Grant/Consume/TransferFungible, MaterializeUnique/TransferUnique/
DestroyUnique, and Equip/Unequip/Swap methods for ordinary operations. Redundant
static InventoryService/ItemService/EquipmentService forwarding APIs are removed.
`InventoryEdit` remains optional for grouped changes such as unequip → transfer →
equip. Failed edits leave the store unchanged; publication rejects stale edits.

After registering an owner's inventory/equipment, attach live facades if useful:

```csharp
var inventory = new InventoryComponent(store, owner);
var equipment = new EquipmentComponent(store, owner);
entities.Add(owner, inventory);
entities.Add(owner, equipment);
inventory.MaterializeUnique(item);
equipment.Equip(item.Entity, slots);
```

These facades retain the store and owner ID, resolving current records on each
read or operation. Retaining the component is safe across edit publication;
individual returned views/lists describe the read that produced them. They do
not hold another ledger, grant writable access to internal maps, or synchronize
EntityStore parent relationships. Grouped operations still use the same store's
`Prepare()` edit. Inventory-only owners need no EntityStore attachment.

For metadata-bearing quantities, give each distinct stack a product-selected
`InventoryStackId` and use `Grant(owner, definition, stackId, quantity)`.
`InventoryView.Stacks` exposes the IDs; their scope is the owner inventory.
Selected-stack Consume, SplitFungible, TransferFungible and MergeFungible all
use the same store ledger and capacity checks. The product chooses compatible
merge targets and copies or retires its metadata using the returned IDs.
Splitting requires a new destination ID and leaves a positive source quantity;
merging retires the source ID. A full transfer can preserve its ID in an owner
where that ID is unused. Partial transfers require an explicit destination ID,
either new or selected for a compatible merge. `MaximumQuantity` limits each
stack; inventory capacity accounts for every stack and unique item together.
Every inventory mutation selects an explicit stack ID. Definition-only Grant,
Consume and TransferFungible overloads and the implicit-ID InventoryStack
constructor have been removed. Existing callers must choose stack IDs; the
Engine never derives one from definition text. Definition-level quantity reads
still aggregate all stacks of that definition.
`InventoryState.CaptureStacks()` and `InventoryState.Restore(...)` retain stack
IDs, definitions and quantities; restore and registration validate capacity.
Persist product metadata keyed by those owner/stack IDs, or map them to save-local
identities and re-grant on rebuild. No second quantity ledger or metadata-encoded
definition ID is needed.

### Explicit capture, restore and live inspection

Capture selected durable values into product-owned records on request. Save those
records through `ProductStateStore<T>.Save`; they should not contain live component
references, native leases, input state or presentation resources. `Load` reads and
decodes the current shape and returns a value; it never changes the live graph.
Build and validate replacement owners from that value, then install them at the
product boundary. A failed decode or candidate build leaves the old owners in place.
This does not promise rollback of independent native work already committed.

Rebuild shared references deliberately. For example, construct one maximum `Stat`,
put it in `StatsComponent.Stats`, and pass that same object to the restored `Track`.
D20 restores this graph through participant admission; its save contains numeric
values and product identities rather than a serialized component graph.

`JsonProductStateCodec<T>` is the ordinary JSON path over `ProductStateStore<T>`: supply
the save type plus a `JsonTypeInfo<T>` (source-generated contexts work under NativeAOT
without reflection) or `JsonSerializerOptions` for CoreCLR convenience, then Save/Load
with no byte-buffer plumbing and no version scaffolding. Absent keys report absent;
malformed bytes fail in deserialization; a JSON null document fails rather than decoding
to a missing value. Custom binary codecs stay available through the same small
`IProductStateCodec<T>` contract.

The direct Persistence requests, receipts and blobs also carry no product schema
version. Storage owns only its file layout marker and revision; specialized codecs
(such as voxel edit history) identify their own payload format during decoding.
The current layout replaces the old schema-bearing envelope without migration;
old development save files must be discarded or explicitly converted by their owner.

`ProductStateStore<T>.Delete(key, guard, expectedRevision)` and the direct
`Persistence.Delete(PersistenceDeleteRequest)` durably remove one scoped key.
`Deleted` reports the removed revision; `Missing` reports zero. Guards match Save:
`Any` accepts either state, `Exact` requires an existing matching revision, and
`Absent` requires no key. A mismatch returns `RevisionConflict` with the current
revision (zero when absent) without removing bytes. Loaded blobs remain readable.
Recreating a deleted key starts at revision one; revisions are not tombstone IDs.
Storage failures throw rather than returning a deletion receipt. After an I/O
failure, reload to determine whether removal occurred. Product slot catalogs and
metadata remain product-owned.


`StatsComponentCapture.Capture` reads a component's selected stat/track values as plain
data and `Rebuild` reconstructs an equivalent set with each track sharing its rebuilt
maximum `Stat` — later stat changes reach the same track. Authored stat sources are not
captured; re-supply them via `SetSources` in the optional `restoreStat` callback,
before any tracks are constructed. That callback runs once per distinct stat and
receives fresh modifier removal handles in capture order; retain them with the
product-owned temporary effects that will later remove those modifiers. Without
the callback, captured local modifiers remain attached for the rebuilt stat's lifetime.
Multiple names for the same Stat or Track are captured as aliases and rebuild to
the same instance. The callback receives the ordinal-first stat name, not each alias.

```csharp
StatsComponent restored = StatsComponentCapture.Rebuild(saved, (capture, stat, handles) =>
{
    stat.SetSources(StatId.Parse(capture.Id), sourcesByStat[capture.Id]);
    restoredModifierHandles[capture.Id] = handles; // product-owned removal associations
});
```

Effects rebuild by re-applying definitions
with fresh instance ids and product provenance through `EffectsComponent.Apply`;
inventory rebuilds by re-registering, granting stacks, materializing uniques under
product-mapped fresh entities, and equipping with re-supplied slot definitions.
Keep an item's saved instance key separate from its definition ID: two swords can
share a definition while remaining different items. The mechanics example assigns
save-local keys, maps each to a fresh runtime entity, and stores every equipment slot
per item (including multi-slot items). These keys are product-owned save data, not an
Engine identity registry. Definitions, provenance, and durable identity mapping remain
product choices.

For debug inspection, opt in on the existing product execution boundary:

```csharp
var debug = new EntityStoreDebugModule();
debug.RegisterStore("session", session.Entities);
debug.RegisterMechanicsProjections(maximumEntries: 16);
// Register this module with the product's debug-command catalog.
// After adopting a restored session:
debug.ReplaceStore("session", restoredSession.Entities);
// Before ending the registration's lifetime:
debug.UnregisterStore("session");
```

`RegisterProjection<T>(formatter)` also supports custom class/value projections
without a numeric descriptor. Descriptor-specific projections remain available
and take precedence for that descriptor. `entity.get` reports component types and
keys for `entity.component`. Every command reads the currently registered store
and current component fields, even when in-place edits leave structural revisions
unchanged. Mechanics projections limit entries and all projection output remains
bounded to 4096 characters. Returned debug metadata is an observation, not a save
or replay checkpoint. Registration is local and explicit; no mechanics discovery
or structural-version value cache is involved.

### Spatial debugging maps

`Spatial.ReadMap` reads a bounded (up to 1,024 cells), world-aligned X/Z map
from a live `SpatialSession`. Supply the minimum X/Z corner, cell size,
columns/rows, a collision Y interval, a separate navigation support Y interval,
and current product-owned dynamic colliders. Columns advance +X; rows advance
+Z. The copied receipt carries spatial publication identity and source,
collision and navigation revisions. It does not change navigation or gameplay.

Collision tests each full cell footprint against retained geometry and enabled,
non-trigger supplied colliders. Navigation samples the cell center against the
retained navigation projection, preserving support counts/heights and traversal
allowance. No navigation sample means unknown, not walkable. A collision miss
means no hit in the supplied/retained sources, not proof of loaded empty space
or character clearance. These are different observations, not a merged occupancy
truth. Multiple support heights remain explicit; this first view is not a full
stacked-floor visualizer.

`Rusty.Engine.Debugging.SpatialMapSnapshot.Capture` combines that read with
product-supplied `SpatialMapObservation` (stamp, player position/facing) and
`SpatialMapAnnotation` values (stable ID, label, relation, state, world position).
Call it at the existing serialized live-debug boundary for coherent product
facts. `ToAscii()` emits map layers and a legend; `ToJson()` emits the same
snapshot with compact row-major cell arrays whose `cellFields` names define
the columns. Both are NativeAOT-compatible. The helper retains copied facts,
not the session or collider inputs. Annotation limits, out-of-view counts, and
height intervals are explicit. The current view is omniscient.

Expose a product command through the ordinary generated debug catalog and use
`runtime-pack/bin/rusty-live-debug --origin http://127.0.0.1:PORT --command
"spatial.map ascii 12 1"` when the product implements that command (as
`rusty-doom` does). The CLI transports the command; the product supplies semantic
annotations and the Engine owns the spatial read. Rendering and fast-controller
experiments are independent of this capability.

### Source GLB material extensions

Live GLB admission preserves `KHR_materials_specular` and `KHR_materials_volume`
through the existing Engine loader, including required declarations. Optional
`FB_ngon_encoding` exporter hints over core triangles are accepted; declaring
that metadata as required is still unsupported. Material textures/factors stay
in the source resource and are realized by the Engine renderer. Unknown required
extensions still produce an import diagnostic without replacing the current view.

### GLB inspection and displayed-pose bounds

`Animation.SetMeshInspection(new(appearance, wireframe, matte, wholeVoxelNormals,
boundsRequest))` selects retained inspection for an animated-mesh appearance,
including GLBs without clips. Publish the appearance in the normal Graphics
snapshot. This appearance-level setting applies to every instance selecting that
appearance; use separate appearances for independent inspection. Inspection
updates preserve playback and target identity. Renderer
instances own temporary material/geometry clones; admitted source resources and
textures remain shared and unchanged. Matte keeps textures and sets PBR roughness
1, metalness 0 and environment intensity 0.35. Whole-voxel normals affect only
predominantly integer unit-face meshes; skinned/morph geometry stays authored.

A changed nonzero `boundsRequest` asks for the displayed pose's world-space bounds
once after the next renderer animation update. `Animation.ReadRealizationFactAt`
returns `MeshInspection` with the logical object, renderer generation,
`BoundsRequest`, `HasBounds`, `BoundsMin`, `BoundsMax`, and `VoxelNormalMeshes`.
No bounds means an empty/unmeasurable mesh, not a zero-sized box. Match the object
and request before consuming; cancel pending camera actions when the user moves
it. A replacement renderer replays the retained request once. Zero disables the
request. This is observation, not a second animation clock or automatic camera.
