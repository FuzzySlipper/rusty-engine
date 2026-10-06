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

Input intents and mappings are `RustyEngineProductInputIntent` and
`RustyEngineProductInputMapping` items. An optional UI-projection identity is
the `RustyEngineProductUiProjectionStream` and
`RustyEngineProductUiProjectionContract` pair of properties; build a
projection's `UiValue` from ordinary JSON with `UiValues.FromJson`. Declaring
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
Engine code and explanation. Generated wrappers copy the native diagnostics
into managed values before throwing, so the exception holds no native memory.
For example, an unadmitted audio clip reports
`CSHARP_AUDIO_CLIP_HANDLE`, and a stale sprite atlas reports
`CSHARP_SPRITE_ATLAS_HANDLE`. An exception escaping a product callback is
reported to runtime diagnostics with its complete text and managed stack trace,
and the host prints one line to stderr naming the callback, the exception type,
its message and the first product stack frame:

```text
rusty: product update faulted: CSHARP_PRODUCT_CALL: Spatial.ProposeCharacterStep returned status 0: EngineCallException: Rusty Engine Spatial.ProposeCharacterStep returned status 0. … (at Game.Movement.Step(…) in …/Movement.cs:line 42)
```

Pass `--diagnostics-log <file>` to `rusty dev` for the full record. An exception
thrown by the product's `Dispose` is printed the same way
(`rusty: product Dispose threw …`).

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
owning service.

### Runtime input remapping

Keyboard mappings use `key-a`–`key-z`, `digit-0`–`digit-9`, `space`, `enter`,
`escape`, `arrow-up`, `arrow-down`, `arrow-left`, `arrow-right`, and the
`shift-left/right`, `control-left/right`, `alt-left/right` pairs. In C#, use
`KeyboardControl.ArrowUp` (and the other directions) or `KeyboardControl.Enter`.
The manifest name for Enter is `enter`, not `key-enter`; arrow names use
`arrow-`, not `key-`. Arrow keys use the same Engine input path as other
keys.

Use `context.Engine.Input.ReplacePhysicalMappings(mappings)` to replace the
whole physical mapping set during product creation, Start, Pause, Resume,
Restart, or an admitted Update callback. The mappings are
`ProductInputMapping` values and must target the product's declared semantic
intents; remapping does not add intents
or change their value kinds or payload contracts. An empty set disables
physical mappings while leaving direct intents available.

`Staged` means the replacement takes effect when the callback finishes, even if
it then throws. The last valid replacement in that callback wins.
`InvalidMappings` leaves the current mapping set and any earlier valid
replacement unchanged. `Unavailable` reports a call
outside those supported callbacks (such as Attach, Shutdown, debug, timeline
completion or paused intents). Duplicate mapping IDs, unknown intents, incompatible value kinds,
and unsupported controls are invalid. Distinct mapping IDs may deliberately share a physical trigger.

At runtime, a successful replacement uses the lifecycle transition or advances
the input control revision and clears held and pending input through the
input lane. During creation it instead selects the initial map before the
lane admits input. Old bindings stop firing, and queued events from the
previous binding cannot trigger stale actions. Products receive the normal
clear fact and must release their derived held state. Focus and text-entry
suppression continue through the same lane. A mapping replacement, pause,
resume, or control replace/release keeps the renderer and its retained world:
only the input, UI and feedback binding moves, and each UI stream's latest
projection is republished under it.
`ProductCreateContext.Input` remains the initial composition snapshot; products
own their chosen settings, UI and persistence.

### Gameplay cursor mode

Keyboard-driven products without mouselook can opt into a free cursor:

```xml
<RustyEngineProductInputCursorMode>unlocked</RustyEngineProductInputCursorMode>
```

The default is `pointer-lock`, for FPS-style mouselook. In `unlocked` mode,
clicking the world view focuses gameplay keyboard input without requesting
pointer lock; pointer movement does not supply camera-look deltas. Marked DOM
controls remain usable, and Engine input still owns focus loss, clearing, and
rebinding. The setting is available as `context.Input.CursorMode` and is
carried in `product.json`. Invalid values reject staging.

A mouselook product can still offer a free-cursor screen (a map, a strategy
view): its UI calls `context.ui.setCursorMode('unlocked')`, which releases
pointer lock and keeps clicks from taking it again, and
`context.ui.setCursorMode('pointer-lock')` to return. `context.ui.cursorMode()`
reads the current mode.

Whenever the pointer is not locked, in either mode, pointer input carries the
cursor position on the Engine canvas, normalized and bottom-left based like a
camera viewport: `PointerButton` events have `HasPosition` set with the
position in `X`/`Y`, and cursor movement arrives as `PointerPosition` events.
A click with the pointer locked has no position. Pass the position to
`CameraQueries.Ray` (with the canvas aspect from `CameraView.ReadSurface`) to
turn a click into a world ray.

### Default lighting

The Engine renderer lights the world and the viewmodel with neutral default
light rigs. A product can disable either rig independently through ordinary
build properties; retained lights created through `Graphics` are unaffected.

```xml
<PropertyGroup>
  <RustyEngineProductDefaultWorldLights>disabled</RustyEngineProductDefaultWorldLights>
  <RustyEngineProductDefaultViewmodelLights>neutral</RustyEngineProductDefaultViewmodelLights>
</PropertyGroup>
```

Each value is `neutral` or `disabled`. These are host defaults, not product
lights: disabling the world rig does not change the viewmodel setting or remove
product-owned point, directional, or spot lights. Invalid values reject staging.

### Output

The runtime draws the world and plays its audio itself. Two build properties
select where, and are carried in `product.json` as `renderer.output` and
`audio.output`:

```xml
<PropertyGroup>
  <RustyEngineProductRenderOutput>window</RustyEngineProductRenderOutput>
  <RustyEngineProductAudioOutput>device-required</RustyEngineProductAudioOutput>
</PropertyGroup>
```

`RustyEngineProductRenderOutput` is `stream` (the default: frames streamed to
the browser shell page) or `window` (a native [desktop window](desktop-shell.md)).
`RustyEngineProductAudioOutput` is `stream` (the default with `stream`
output: the pages watching the frames play the sound), `device-optional` (the
default with `window` output: the runtime's audio device plays it, and a
machine without one runs silent) or `device-required` (the load fails without
a device). `stream` needs `stream` output. `rusty dev --output` and `--audio-output` set them for one launch.
Invalid values reject staging.

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

`Files` exposes the whole snapshot as UTF-8 `Path` and `Bytes` pairs. Content
is an eagerly admitted memory snapshot copied across the generated boundary;
these helpers add no filesystem reads, streaming, parsing framework or writable
store. Treat retained path/payload memory as read-only. A loose content edit
under `rusty dev` replaces the runtime to supply a new snapshot; bundle content
(below) reloads without one.

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

Declared bundle files are excluded from the `ProductContent.Files` snapshot
and its named reads. Discovery
reads only the inventory. Opening a bundle reads that collection's files into
an immutable Rust snapshot, checking each length against the inventory; the
inventory's SHA-256 is each file's identity and is not recomputed. It does not
load other bundles or copy
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
change resolution. After admission, the Engine resource retains its own payload
and does not need the source reference or bundle. Appearance keeps imported
results for the current service lifetime so repeated opens of the same admitted
source avoid decoding, packing and hashing again. Reuse checks immutable buffer
identity, including each GLB dependency. A newly admitted source snapshot is
imported afresh; service reload discards these derived results. Resource handles
still acquire and release their own per-open ownership.

### Content containers installed beside the product

Content that ships on its own (a ruleset, an asset set or a campaign, each
installed or replaced independently of the product) is a content container:
the [release container](#release-container) format holding just one
directory's files, with no manifest, UI or code. Pack one with:

```bash
rusty pack-content modules/srd-ruleset --output library/srd-ruleset.rpak --compress
```

A product or its tool packs the same container in process, with no `rusty` to
find:

```csharp
ProductContentBundle.PackContainer(engine.Content, "modules/srd-ruleset", "library/srd-ruleset.rpak", compress: true);
```

Every file keeps its directory-relative path, length and SHA-256; `--compress`
(`compress`) works as for `--pack`. An output inside the directory throws
`EngineCallException` with `PRODUCT_PACK_OVERLAP`. The file appears by rename once complete, so replace an
installed container by packing or copying a new file and renaming it into
place, never by rewriting it while a product has it open.

A running product opens a container it located, at any filesystem path it
chose (a module library, a download location), as an ordinary bundle:

```csharp
using ProductContentBundle module = context.Content.OpenContainer(path);
ContentSha256 identity = module.Identity;
var manifest = module.ReadText("module.json"); // the product's own format
using ContentReference wall = module.OpenReference("walls/stone.png");
```

Everything in the bundle section above applies: bundle-relative paths, reads,
`OpenReference`, the typed content consumers, GLB dependencies resolved inside
the same container, and references that outlive the bundle. Opening maps the
file and checks its header and inventory; each file's bytes are read when first
used and kept for that open container, so opening every installed container to
read its manifest and its `Entries` is cheap. `Identity`, also on build
bundles, is the SHA-256 over each file's path and SHA-256 in path order: it
follows the files, not how they are stored. A missing file, a file that is not
a container, a truncated one or an inconsistent inventory throws
`EngineCallException` with `PRODUCT_SOURCE_IO`,
`PRODUCT_CONTAINER_NOT_A_CONTAINER`, `PRODUCT_CONTAINER_TRUNCATED` or
`PRODUCT_CONTAINER_CORRUPT`, naming the file. A compressed file that no longer
decompresses is found when it is read: that read (`ReadFile`, `OpenReference`,
`ResolveReference`, or a GLB dependency) throws `EngineCallException` with
`PRODUCT_CONTAINER_CORRUPT`, naming the container and the file. Opening works the same whether
the product itself runs loose under `rusty dev` or from a release container.

What a module is, its ID, version and requirements, which containers to open
and in what order are the product's: the Engine knows only containers and their
identities. A product's own tool packs and opens containers the same way, through
`ProductContentBundle.PackContainer` and `OpenContainer(host.Engine.Content, …)` inside an
`EngineTestHost` call (see [tools](csharp-sdk.md#tools)).

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

The animated-mesh path preserves materials, textures, skins and clips;
`Animation.ReadMeshInfo(resource)` exposes admitted bounds and
material/joint/clip counts; `ReadClips(resource)` copies each clip ID, name and
duration.

`Animation.UpdateAnimatedMeshMaterialFactors` gives one appearance its own
factors for embedded material slots, keeping the slots' textures, maps and
texture transforms. Each `MeshMaterialFactors` names a slot and replaces its
`baseColorFactor` (linear RGBA, 0 to 1) and/or its `emissiveFactor` (0 to 1)
and emissive strength (0 or more). The textures still multiply them. Several
appearances of one resource each carry their own set, so one GLB can take
every palette a level uses without a second decoded copy. Each call replaces
the appearance's whole set; an empty set restores the GLB's own factors. A
change updates the drawn instance in place. A slot marked
`KHR_materials_unlit` draws no emission, overridden or not.

```csharp
engine.Animation.UpdateAnimatedMeshMaterialFactors(new(appearance, new MeshMaterialFactors[]
{
    new(MaterialSlot: 0, OverrideBaseColor: true, BaseColor: new(0.8f, 0.3f, 0.2f, 1),
        OverrideEmission: true, EmissiveFactor: new(1, 0.6f, 0.2f), EmissiveStrength: 2),
}));
```

Use the ordinary instance/playback APIs for animation. Dispose
instances, publish the snapshot without their appearances, then dispose
appearances and resources. Direct-instance teardown also accepts an already
published removal snapshot and sends no stop to that removed target. The source
reference can be disposed immediately after resource admission. Transient
animation imports do not enter the startup-source import cache, so closing the
reference and resource releases these snapshots. A malformed or incomplete GLB throws `EngineCallException`
with operation diagnostics without failing the surrounding product callback.
Admit the replacement before releasing the previous selection to keep it visible
when an import fails.

Use the same asset admissions during Create or a later product update:

| Asset | Bundle consumer | Format |
| --- | --- | --- |
| Images and textures | `Graphics.OpenResourceFromContent` | RGBA PNG |
| Packed static geometry | `Graphics.OpenResourceFromContent` | `.rmesh` |
| Authored static mesh | `Graphics.CreateStaticMeshFromContentReference` | StaticMeshAsset JSON with inline payload, or its [binary form](#binary-static-meshes) |
| Animated meshes and animation packs | `Animation.OpenAnimatedMeshFromContent`, `OpenAnimationClipPackFromContent` | GLB, including same-bundle relative dependencies |
| Fonts | `Graphics.OpenResourceFromContent` | WOFF2 |
| Audio clips | `Audio.OpenClipFromContent` | WAV, Ogg Vorbis, Ogg Opus, MP3, FLAC ([memory policy](recorded-audio.md)) |
| Full-viewport video | `Video.Play`, `Video.Stop`, `Video.Skip` | WebM (`video/webm`; VP9+Opus or video-only VP9) |
| Voxel assets, objects and annotations | `VoxelContent.LoadAssetFromContent`, `LoadObjectFromContent`, `LoadAnnotationFromContent` | Typed JSON formats |
| Imported voxel models | `VoxelContent.LoadMagicaVoxelFromContent` | MagicaVoxel `.vox` |
| Authored catalogs/prefabs/scenes and spatial artifacts | Typed `ContentReference` consumers | Their Engine document formats |
| Text and arbitrary bytes | Bundle `ReadText`, `ReadBytes`, `ReadDirectory`, or Content reference reads | No asset decoder required |

The bundle is a source container; an admission still applies the relevant
Engine format rules. It does not make arbitrary image, model or audio formats
supported. The Engine hands admitted resource bytes to its renderer, audio and
video, including assets first loaded after startup. Products do not extract
bundle files or build renderer URLs.

Missing bundles/files report their logical names. A bundle whose file lengths
do not match its staged inventory fails to open; rebuild/restage it. Bundles
are directories in a staged Product and inventory entries in a [release
container](#release-container); both open the same way. Closing a collection does not
free independent GPU resources or force managed garbage collection. Bundle
discovery does not produce URLs for DOM images or fonts.

To show a bundle's or container's PNG in DOM UI (a portrait, an item icon),
grant it to the UI: `UiImage image = engine.Ui.OpenImage(new
UiImageRequest(reference))` takes an open `ContentReference` whose bytes are a
PNG (else `CSHARP_UI_IMAGE_NOT_PNG`) and keeps them, so the reference and its
bundle may be released. `image.Url()` is a same-origin URL the product puts in
its projection for an `<img src>`; the host serves the PNG there until the
product disposes the image, and answers 404 afterwards. The UI never receives
image bytes through a projection.

A font for DOM UI (a skin's TTF, OTF, WOFF or WOFF2) is granted the same way:
`UiFont font = engine.Ui.OpenFont(new UiFontRequest(reference))` refuses
anything else with `CSHARP_UI_FONT_FORMAT`, and `font.Url()` is a same-origin
URL for a CSS `@font-face` `src`, served with the font's content type until
the font is disposed.

### Binary static meshes

A large static mesh can skip the JSON parse. The same call admits a binary
file that carries the JSON form's static mesh document, with its payload read
from the Engine's packed mesh resource that follows it, recognised by its first
eight bytes. Little-endian:

| Field | Encoding |
| --- | --- |
| Magic | `RSTATMSH` (8 bytes) |
| Descriptor length | u32 |
| Descriptor | the static mesh JSON document, its `payload.source` a `resource` source: `resource` `mesh-resource/<hex>`, `contentHash` `sha256:<hex>` of the resource bytes, `byteLength`, `encoding` and each stream's byte offset |
| Packed mesh resource | `byteLength` bytes, to the end of the file |

The packed mesh resource is the Engine's `.rmesh` body: an 8-byte magic
(`RMSHLE01` for positions and normals, `RMSHLE02` with UVs, `RMSHLE03` with
colors; encoding `packedStreamsLeV1`/`V2`/`V3`), its total byte length and its
payload count (1) as u32, then f32 positions, normals, UVs and colors and u32
indices. Validation, refusal codes and the admitted renderer resource are those
of the JSON form; a changed stream fails its `contentHash`
(`CSHARP_STATIC_MESH_PACK`). A town-sized mesh of 100,000 vertices is 3.8 MB
instead of 6.0 MB of JSON, and imports in about 3.5 ms instead of 41 ms.
[`triangle.rstatmsh`](../fixtures/csharp-static-mesh/triangle.rstatmsh) is
`triangle.static-mesh.json` in this form.

## Run and package

`rusty install` installs the pinned [SDK/runtime pair](csharp-distribution.md):
the SDK feed and runtime pack together. Ordinary consumption does not need an
Engine checkout, Cargo, binding generation, or copied Engine browser files.
The development command runs the product on the pinned pair's runtime:

```bash
rusty dev --project src/Example.Game/Example.Game.csproj
```

With `<RustyEngineProject>src/Example.Game/Example.Game.csproj</RustyEngineProject>`
in the repository's `Directory.Build.props`, plain `rusty dev` and `rusty build`
run that project; `--project` still selects another.

It restores, builds and stages the ordinary project in one MSBuild invocation
with no nested build, then launches the packaged host through CoreCLR. Staging
copies only changed UI and content, removes deleted files, and writes
`product.json` last.

The declared inputs are `RustyEngineWatchPaths`. By default the SDK declares
the project directory, the UI source root, the content root, and the directory
of every project the product references, directly or transitively. The Engine
projects that a source-development override references are not declared. A
product that sets `RustyEngineWatchPaths` declares the whole list itself.
Beneath each path, `rusty dev` skips `bin`, `obj`, `node_modules` and other
build or tool directories.

When declared inputs change, `rusty dev` routes the edit:

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
- **Anything else** (C# in the product or a referenced project, a project
  file, loose content) restages the whole
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

A TypeScript UI types what the Engine passes it against the pair's own
declarations. Pass `$(RustyEngineProductUiTypes)` to `tsc` as one more input
(or list it in the UI tsconfig's `files`), then import from the two modules it
declares:

```ts
import type { RustyApplicationUiMount } from '@rusty-engine/product-ui';
import { mountLiveDebugPanel } from '@rusty-engine/live-debug';

export const mountProductUi: RustyApplicationUiMount = (root, context) => { /* ... */ };
```

`@rusty-engine/product-ui` holds the mount signature, the context ports
(`RustyApplicationUiContext`: `ui`, `projection`, `intents`, `input`) and the
projection envelope. A claim's product payload takes the product's own typed
data. `@rusty-engine/live-debug` is resolved at run time by the shell's import
map. Add the file to `RustyEngineProductUiInput` so a pair move rebuilds the
UI.

The SDK runs the command with MSBuild `Exec`: under `cmd.exe` on Windows and
`/bin/sh` on Linux. Keep it one invocation both run, with quoted paths and no
shell utilities, globs or `bash`. Run tools through node by path
(`node "…/node_modules/typescript/bin/tsc"`, `node <script>.mjs`): `npm exec`
needs the `.cmd` shims only a Windows install writes, so it fails on a checkout
installed from Linux. Copy files (the pair's declarations, stylesheets) with
MSBuild in a target that runs `BeforeTargets="BuildRustyEngineProductUi"`. A
copy into the source tree is made only when its content differs, since two
machines sharing a checkout see the pair's file with different times:

```xml
<Target Name="CopyEngineUiTypes" BeforeTargets="BuildRustyEngineProductUi">
  <PropertyGroup>
    <_EngineUiTypesCopy>$(MSBuildProjectDirectory)/../ui/engine-types/rusty-engine-product-ui.d.ts</_EngineUiTypesCopy>
  </PropertyGroup>
  <GetFileHash Files="$(RustyEngineProductUiTypes)">
    <Output TaskParameter="Hash" PropertyName="_EngineUiTypesHash" />
  </GetFileHash>
  <GetFileHash Files="$(_EngineUiTypesCopy)" Condition="Exists('$(_EngineUiTypesCopy)')">
    <Output TaskParameter="Hash" PropertyName="_EngineUiTypesCopyHash" />
  </GetFileHash>
  <Copy SourceFiles="$(RustyEngineProductUiTypes)" DestinationFiles="$(_EngineUiTypesCopy)"
        Condition="'$(_EngineUiTypesHash)' != '$(_EngineUiTypesCopyHash)'" />
</Target>
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
renderer owns non-UI presentation.

### Release container

A release ships the Product as one container file instead of the loose
directory:

```bash
rusty build --project /path/to/Example.Game.csproj --pack release
rusty-product-host --product release/product.rpak --loader coreclr
```

`--pack` (with or without `--aot`) stages as usual, then writes
`release/product.rpak` holding `product.json`, `ui/` and `content/`, and copies
`coreclr/` or `native/` loose beside it: hostfxr and the dynamic loader take
file paths. The container is a header, each file's bytes at a 64-byte-aligned
offset, and an inventory of path, length, SHA-256 and bundle membership; the
host maps it read-only and reads the manifest, UI, content and bundles from it
as it reads a loose directory. The inventory's SHA-256 is each file's identity;
there is no second checksum or version field (a pair reads its own output). A
missing magic, a truncated file or an inconsistent inventory stops the host at
start with `PRODUCT_CONTAINER_NOT_A_CONTAINER`, `PRODUCT_CONTAINER_TRUNCATED`
or `PRODUCT_CONTAINER_CORRUPT`. `rusty dev` keeps the loose directory, so a UI
or bundle restage still reloads in place.

Files are stored raw, so the host borrows them from the mapped file. `--pack
--compress` stores each file that zstd shrinks by at least a tenth compressed
instead (JSON and other text; images, audio and video stay raw), and the host
decompresses it when it reads the file. That suits a product with much JSON
content: it trades a smaller release for decompression at startup.

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
runtime pack and passes `RustyEngineUseSourceDevelopment=true` plus the
absolute `RustyEngineSourceDevelopmentPath` to MSBuild; the SDK then references
the checkout's `Rusty.Engine` and product generator projects. The
`Rusty.Engine` package reference must set
`<ExcludeAssets Condition="'$(RustyEngineUseSourceDevelopment)' == 'true'">compile;runtime</ExcludeAssets>`,
or the build stops with an error naming it. Never make this override, an
adjacent checkout, or downstream binding generation the normal product setup.

The fixtures in this repository are provider proof scaffolding. They are
useful when changing the ABI/generator/runtime, but they are not a template
for a downstream repository's launch topology.

### Camera views under UI elements

`context.viewport.anchor(name, element)` ties a camera the product anchors
under `name` to a UI element, and `context.ui.scale()`/`setScale()` read and
set the UI scale ([views that follow the product UI](csharp-lifecycle.md#views-that-follow-the-product-ui)).

### Pause and resume from product UI

`context.lifecycle` pauses and resumes the Engine runtime. `pause()` and
`resume()` resolve to the Engine's answer, `{ accepted, state, code?,
diagnostic? }`. A request for a runtime that has since been restarted or
replaced is not accepted. They reject when the host has failed or is disposed.
`state()` and `subscribe(listener)` follow the state the Engine reports
(`'running'`, `'paused'` and so on), including a pause made elsewhere, so a
pause menu shows what the Engine did rather than what was clicked. Pause stops
simulation and the product's `Update` and runs `Pause`. Held and pending
gameplay input is cleared and is not replayed on resume, and paused time does
not catch up. A `context.intents.claim` made while paused reaches the
product's `HandlePausedIntents` once
([actions while paused](csharp-lifecycle.md#actions-while-paused)), so a pause
or inventory menu can act and show the product's answer. To return to play after `resume()`, call
`context.ui.setInteractionMode('gameplay')` and `context.ui.focusGameplay()`.
`fixtures/csharp-controller-interaction` has a minimal Pause/Resume control.

### Controller input in product menus

With selected-controller input enabled, `mountUi` receives an optional
`context.input.subscribe(observer)` port. The host controller cadence
delivers immutable `{ context: 'interface', fact }` observations while
`context.ui.setInteractionMode('interface')` owns input. Facts use the Engine's
normalized `controller-button` pressed/released edges, `controller-axis`
samples, and `controller-button-value` pressure changes. UI owns their menu
meaning; these are not Rust-mapped gameplay intents. Unsubscribe on UI disposal.

Interface observations never enter the gameplay queue or consume its sequence
numbers. Use `context.intents.claim` for actions that need product processing;
it is the sole ordered command lane. No downstream gamepad polling or
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

The attachment assertion requires `defineMaterial`, `create`, and
`replaceMeshPayload` together in one retained frame, then checks that a second
attachment preserves the voxel baseline and runtime readout. Its failure
reports observed and missing operation kinds for each frame (or that no frame
was published). The fixture builds that baseline during construction/Start
with a `Spatial` session, nonempty `Voxel` edits, bound material slots and a
retained `VoxelScenePresentation.ProjectSceneDirectional` projection. This is
a fixture expectation, not a universal shape for valid graphics.

A product with static meshes, sprites, or no mesh content should not add dummy
voxels or fixture callbacks to pass this check. Launch normally with
`rusty dev --project <product.csproj>`, or run the matched host with
`rusty-product-host --product <staged-Product-directory|product.rpak> --loader coreclr`
without `--exercise`. Verify the product's actual startup and interactions
through that host; fixture success is not evidence of gameplay correctness.

### Requested scene shadows

Set `RustyEngineProductSceneShadows` to `enabled` to render scene shadow maps
for lights with `LightShadowIntent.Requested`. The default is `disabled`.
This writes `renderer.lighting.shadows` in the staged product manifest and
applies to streamed and native-window output, including skinned meshes and
joint-attached meshes. Only requesting lights allocate maps: four cascades for
a directional light, one map for a spot light, six for a point light and one
sky layer for an ambient light. A directional light's cascades follow the
camera out to its `Range` (100 m by default). Set
`RustyEngineProductShadowBudget` to a number of shadow layers to have the
Engine choose which requesting lights cast, by `ShadowPriority` then distance
(`renderer.lighting.shadowBudget`; 0, the default, for no limit). See
[shadows](lighting-and-sky.md#shadows) for resolution, softness and the
budget.
