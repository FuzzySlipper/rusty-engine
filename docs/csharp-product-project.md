# C# product project, build and staging

The ordinary product project: package reference, product facts, bundled files, staging, the NativeAOT check and the host exercise contract. Run the product with `rusty dev`; see the [C# SDK guide](csharp-sdk.md).

## Build a product through the packaged surface

`Rusty.Engine` is one immutable NuGet package containing the public C# service
surface, managed helpers, generated contracts, and the product generator. An
ordinary Product repository pins one pair in its `Directory.Build.props`, which
also declares that pair's feed (see [distribution](csharp-distribution.md#use-a-pair-from-a-product)),
and references only the package, exactly:

```xml
<ItemGroup>
  <PackageReference Include="Rusty.Engine" Version="[$(RustyEnginePackageVersion)]" />
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
the corresponding `RustyEngineProduct*` MSBuild items/properties. Declaring
`RustyEngineProductEntryType` makes that project the product root: the SDK's
generator adds its bind export and debug catalog to the project's own
compilation, and the project builds as a CoreCLR component (runtimeconfig and
dependencies beside the assembly). There is no second generated project. A
Product must not check in a `NativeProduct` bridge, generated bindings,
exports, or service-table code.

Product code implements `IEngineProduct`, accepts `ProductCreateContext`, and
keeps `IEngineContext` or the named services it needs. Exactly one concrete
`RustyEngineProductEntryType` is declared. The generator supplies both CoreCLR
and NativeAOT bind export without assembly scanning or product-side
registration infrastructure. The export is safe C#; the interop, service
implementations and product lifetime are compiled into `Rusty.Engine`, so a
product project needs no `AllowUnsafeBlocks`.

### Diagnosing native service refusals

Catch `EngineCallException` at the product boundary and inspect `Service`,
`Operation`, `Status`, and `Diagnostics.Span`. Its message includes each returned
Engine code and explanation. Generated wrappers copy and release the native
operation receipt before throwing, including owned-resource disposal and
borrowed/span request shapes; products retain only managed diagnostic values.
Audio, Graphics, Animation, Presentation, CameraView, Dynamics, Video, UI
stream lifetime, and implicit-field operations preserve their recorded native
refusal reasons. For example, an unadmitted audio clip reports
`CSHARP_AUDIO_CLIP_HANDLE`, and a stale sprite atlas reports
`CSHARP_SPRITE_ATLAS_HANDLE`. An exception escaping a product callback is
reported to runtime diagnostics with its complete text and managed stack trace.

A refusal is operation-local: the refused operation leaves Engine state as it
was, and its exception is the only consequence. A product that catches
`EngineCallException` may continue the callback, and its other output is
published normally. An exception that escapes the callback does not undo what
the callback already did: that output is published too, and the runtime faults
the lifecycle. Simulation stops with the product loaded so you can inspect it;
resume (for example through `playtest` or the live-debug lifecycle route)
continues the same product, and restart resets it. The process is not replaced.
The
[caught-refusal fixture](../fixtures/csharp-caught-refusals/CaughtRefusalChecks.cs)
catches Graphics, Audio, CameraView, Dynamics and UI refusals, then performs
ordinary work in the same callback. Resource release still uses the exact
owning service. Adopt the matching SDK **and** runtime: these receipts change
the native function table.

### Runtime input remapping

Keyboard mappings use `key-a`–`key-z`, `digit-0`–`digit-9`, `space`, `enter`,
`escape`, `arrow-up`, `arrow-down`, `arrow-left`, `arrow-right`, and the
`shift-left/right`, `control-left/right`, `alt-left/right` pairs. In C#, use
`KeyboardControl.ArrowUp` (and the other directions) or `KeyboardControl.Enter`.
The manifest name for Enter is `enter`, not `key-enter`; arrow names use
`arrow-`, not `key-`. Browser `ArrowUp/Down/Left/Right` events use the same
Engine input path as other keys. Consume a matching SDK/runtime pair.

Use `context.Engine.Input.ReplacePhysicalMappings(mappings)` to replace the
whole physical mapping set during product creation, Start, Pause, Resume,
Restart, or an admitted Update callback. The mappings use the existing
`ProductInputMapping` values and must target the product's declared semantic intents; remapping does not add intents
or change their value kinds or payload contracts. An empty set disables
physical mappings while leaving direct intents available.

`Staged` means the replacement takes effect when the callback finishes, even if
it then throws. The last valid replacement in that callback wins.
`InvalidMappings` leaves the current mapping set and any earlier valid
replacement unchanged. `Unavailable` reports a call
outside those supported callbacks (such as Attach, Shutdown, debug or timeline
completion). Duplicate mapping IDs, unknown intents, incompatible value kinds,
and unsupported controls are invalid. Distinct mapping IDs may deliberately share a physical trigger.

At runtime, a successful replacement uses the lifecycle transition or advances
the input control revision and clears held and pending input through the
existing input lane. During creation it instead selects the initial map before
the lane admits input. Old bindings stop firing, and queued events from the previous binding cannot trigger stale actions.
Products receive the normal clear fact and must release their derived held
state. Focus and text-entry suppression continue through the same lane.
A mapping replacement, pause, resume, or control replace/release keeps the
browser renderer and its retained world: only the input, UI and feedback
binding moves, and each UI stream's latest projection is republished under it.
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
store. Treat retained path/payload memory as read-only. A loose content edit
under `rusty dev` replaces the runtime to supply a new snapshot; bundle content
(below) reloads without one. Tauri and sealed-container host
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
source for the call and copies it once into managed storage, with no
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
| Audio clips | `Audio.OpenClipFromContent` | WAV, Ogg Vorbis, Ogg Opus, MP3, FLAC ([memory policy](recorded-audio.md)) |
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

`rusty install` installs the pinned [SDK/runtime pair](csharp-distribution.md):
the SDK feed and runtime pack together. Ordinary consumption does not need an
Engine checkout, Cargo, binding generation, or copied Engine browser files.
The development command runs the product on the pinned pair's runtime:

```bash
rusty dev --project src/Example.Game/Example.Game.csproj
```

It restores, builds and stages the ordinary project in one MSBuild invocation
with no nested build, then launches the packaged host through CoreCLR. Staging
copies only changed UI and content, removes deleted files, and writes
`product.json` last. When declared inputs change, `rusty dev` routes the edit:

- **UI or content-bundle edits only.** These are files under
  `RustyEngineProductUiSourceRoot`, `RustyEngineProductUiRoot`, or a
  `RustyEngineContentBundle` root. `rusty dev` runs only
  `StageRustyEngineProductAssets` (no C# build) and the running runtime
  reloads them. The product keeps running.
  - UI is served `no-store`, so the next page load gets the new files.
  - The next `OpenBundle` sees edited, added and deleted bundle files under
    their new content identities. Bundles and references opened earlier keep
    their bytes.
  - If the stage or the reload fails, the old UI and bundle inventory stay in
    place until the next edit.
- **Anything else** (C#, the project file, loose content) restages the whole
  Product and replaces the runtime. Loose content is the create-time snapshot
  described above.

A product whose UI is compiled declares the compiler command once. The SDK
runs it only when a file under the UI source root, the project file, or an
added `RustyEngineProductUiInput` is newer than the last successful build, or
the UI entry output is missing. An unchanged or C#-only build therefore skips
the UI compiler:

```xml
<PropertyGroup>
  <RustyEngineProductUiRoot>$(MSBuildProjectDirectory)/../ui/generated</RustyEngineProductUiRoot>
  <RustyEngineProductUiSourceRoot>$(MSBuildProjectDirectory)/../ui</RustyEngineProductUiSourceRoot>
  <RustyEngineProductUiBuildCommand>pnpm --dir "$(MSBuildProjectDirectory)/../.." exec tsc --project "$(MSBuildProjectDirectory)/../ui/tsconfig.json"</RustyEngineProductUiBuildCommand>
</PropertyGroup>
<ItemGroup>
  <!-- Dependencies outside the UI source root. -->
  <RustyEngineProductUiInput Include="$(MSBuildProjectDirectory)/../../package.json;$(MSBuildProjectDirectory)/../../pnpm-lock.yaml" />
</ItemGroup>
```

Do not hook a UI compiler onto the SDK's staging or validation targets
yourself; that reruns it on every C# edit. `--bind-host`, `--port`, and
`--live-debug` override the corresponding staging properties for a development
session. Use `--debugger` for managed breakpoint sessions; see
[CoreCLR diagnostics](coreclr-diagnostics.md) for runtime process discovery,
profiling, and the debugger's startup deadline.

For explicit staging without launching, run
`rusty build --project /path/to/Example.Game.csproj` (the SDK target
`StageRustyEngineCoreClrProduct`).
A plain `dotnet build` produces a loadable product assembly but does not stage
it. Use the target (or `rusty dev`) to regenerate `obj/Rusty.Engine/Product`.
Staging runs `ValidateRustyEngineProduct`, the project's ordinary `Build`, and
`StageRustyEngineProductAssets` in that one invocation. A compiled UI uses
`RustyEngineProductUiBuildCommand` as shown above.

The Product directory has `product.json`, the project's build output under
`coreclr/` (the product assembly keeps its own name), and Product-owned `ui/`
and `content/`. Each stage copies the build output into a fresh `coreclr/`
directory, because a running worker keeps the previous files mapped until the
supervisor replaces it. Engine JavaScript and host binaries stay in
the runtime pack. Product UI is DOM UI and accessibility only; the Engine
renderer remains the owner of non-UI presentation.

The package and runtime pack carry exact generated ABI identities. A mismatch
is rejected before product construction. Use a package and runtime pack built
from the same Engine release; do not add version negotiation, copy a host into
the Product, or repair the mismatch with handwritten interop.

NativeAOT is an explicit fidelity/release check, not the edit-run loop:

```bash
rusty build --project /path/to/Example.Game.csproj --aot
```

(`rusty build --aot` runs the SDK target `VerifyRustyEngineAot`.)

It publishes the project for `linux-x64` as a NativeAOT shared library and
stages `native/<Product>.so` with the Product UI and content. It does not build
or stage CoreCLR, and its `product.json` lists only `nativeAot`. Each staging
target's manifest lists exactly the runtime artifacts that target staged. To
run both loaders against one bundle (for example, to compare them), use
`-t:StageRustyEngineCombinedProduct`, which stages both and lists both.

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
