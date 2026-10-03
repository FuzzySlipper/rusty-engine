# C# SDK guide

A Rusty Engine product is an ordinary C# project that references the
`Rusty.Engine` package and runs on the matching Engine runtime. The `rusty`
command owns the workflow; `rusty --help` and `rusty <command> --help` are
the reference. This page is the short path through it, with links to the
capability references.

## Start

Prerequisites: Linux x64 or Windows x64, the .NET 10 SDK, `curl` and `tar`
(with `xz` for window mode). The CLI and the feed declaration below find the
shared cache at `.cache/rusty-engine` under `HOME`, or `USERPROFILE` on
Windows. Get `rusty` once (rerun to refresh it):

```bash
curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.ps1 | iex
```

Start a new product from [rusty-template](https://github.com/FuzzySlipper/rusty-template),
which is kept on a current pair. An existing product needs one pin plus its feed
declaration in `Directory.Build.props` and exact package references:

```xml
<Project>
  <PropertyGroup>
    <RustyEnginePackageVersion>0.1.0-dev.abc123def456</RustyEnginePackageVersion>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == '' and '$(XDG_CACHE_HOME)' != ''">$(XDG_CACHE_HOME)/rusty-engine</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == '' and '$(HOME)' != ''">$(HOME)/.cache/rusty-engine</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(USERPROFILE)/.cache/rusty-engine</RustyEngineCache>
    <RestoreAdditionalProjectSources>$(RestoreAdditionalProjectSources);$(RustyEngineCache)/pairs/$(RustyEnginePackageVersion)/sdk-feed</RestoreAdditionalProjectSources>
  </PropertyGroup>
  <Target Name="RequireRustyEnginePair" BeforeTargets="Restore;_GenerateRestoreGraph" Condition="!Exists('$(RustyEngineCache)/pairs/$(RustyEnginePackageVersion)/sdk-feed')">
    <Error Text="Rusty Engine pair $(RustyEnginePackageVersion) is not installed: run `rusty install` in this repository." />
  </Target>
</Project>
```

```xml
<PackageReference Include="Rusty.Engine" Version="[$(RustyEnginePackageVersion)]" />
```

Plain `dotnet build`, `dotnet test` and `dotnet run` resolve the same pinned
SDK. The project also declares its entry type, product facts, UI and content
roots; see [product project, build and staging](csharp-product-project.md).

## Run

From the product repository:

```bash
rusty status
rusty install
rusty dev --port 8787
```

`rusty dev`, `rusty dev start|stop|status` and `rusty build` run the project
`--project` names, else the repository's default: the nearest
`Directory.Build.props` at or above the current directory names it, relative to
itself, beside the pin. Either path separator works on Windows and Linux:

```xml
<RustyEngineProject>src/Game/Game.csproj</RustyEngineProject>
```

`rusty dev` builds and stages the product, runs it on CoreCLR, and watches the
declared inputs: by default the product project, its referenced projects, the
UI source and the content (see
[run and package](csharp-product-project.md#run-and-package)). UI and
content-bundle edits reload into the running product; C#, project and
loose-content edits rebuild and replace the runtime. Useful flags:
`--live-debug`, `--debugger` (managed breakpoints, see
[CoreCLR diagnostics](coreclr-diagnostics.md)) and `--headless`.

Scripts and agents run it in the background, one session per project:
`rusty dev start` (same options) returns once the product serves
and prints `{url, port, pid, runtimeInstanceId, persistenceRoot, log}` as JSON,
or exits nonzero with the log's tail. `rusty dev stop` disposes the
product as Ctrl+C would, and `rusty dev status` reports the
session. Both find it through `.runtime/dev/`, which has one directory per
project path in the repository, so they never need `kill` or `pkill`. `start` needs a pinned pair that has it.

The runtime renders the world itself with `render-wgpu`. By default it streams
frames to the browser shell page that `rusty dev` serves. The product runs
from load either way. The runtime draws only while a page watches that stream,
but animation and video completions do not wait for one: unwatched, they
advance on Engine time without drawing. `--headless` opens a page in headless
Chromium (`--chromium <executable>` selects it) for an unattended run that
wants frames drawn or the product UI mounted. `RustyEngineProductRenderOutput`
set to `window` in the product project (or `rusty dev --output window` for one
launch) presents to a native window instead; the first such run downloads the
pair's desktop runtime pack into the cache (see
[desktop shell](desktop-shell.md)). `RustyEngineProductAudioOutput` set to
`device-required` (or `--audio-output device-required`) fails the load when no
audio device opens.

`rusty build` stages without running; `--aot` also publishes the
NativeAOT product, an explicit fidelity/release check rather than the edit
loop.

### One checkout on two machines

A machine that runs a checkout another machine also builds (Windows on a
network drive such as `P:` while Linux edits it) keeps its own output out of
the checkout. In that machine's cache `config.json` (beside `pairs/`; `rusty
status` prints the cache):

```json
{"localOutput": "C:/rusty-local"}
```

`rusty dev`, `rusty dev start|stop|status` and `rusty build` then keep this
checkout's session records, logs and persistence in
`<localOutput>/<checkout>/runtime` and pass MSBuild
`ArtifactsPath=<localOutput>/<checkout>/artifacts`, so restore, `bin`, `obj`
and the staged product of the project and every project it references go
there. `rusty status` prints both locations. Source, content and UI sources
stay in the checkout and edits from either machine reload as usual. A plain
`dotnet build` there needs `--artifacts-path` with that directory. A UI build
command writes its output where the product points `RustyEngineProductUiRoot`;
pointing it under `$(BaseIntermediateOutputPath)` keeps that output per machine
too. `rusty dev` runs the pinned pair's own CLI, so the pair must have this.

The share and the product's packages must also serve both machines:

- A Samba share needs `oplocks = no` and `level2 oplocks = no`. Edits made on
  the Linux side do not break the leases Samba otherwise grants, so Windows
  keeps stale file times and never reloads them.
- Windows does not run or load programs from a Samba share whose files carry
  no execute permission. Keep build output local (`localOutput` above, or
  `-p:ArtifactsPath=<local dir>` for a tool a product runs with `dotnet run`).
- One package install serves both systems only when its packages are plain
  JavaScript or carry both platforms' native builds. pnpm's
  `supportedArchitectures` (`os: [current, win32]`) fetches both. pnpm's
  symlinked `node_modules` cannot be read from Windows over Samba; such a
  product uses `nodeLinker: hoisted`.

## Test

A product's test project exercises its Engine calls against the Engine's own
services with `Rusty.Engine.Testing.EngineTestHost`: the same service set a
running product receives, in process, with no renderer, browser, window or
audio device. Ownership, admission and handle rules refuse exactly as at run
time, with the same `EngineCallException` diagnostic codes. For example,
releasing a render resource that a live sprite atlas uses refuses with
`CSHARP_RENDER_RESOURCE_IN_USE`, a collision residency request for an absent
asset with `CSHARP_COLLISION_REPLACE`, and separate Persistence scopes get
separate stores.

```csharp
using var host = EngineTestHost.Create(new EngineTestHostOptions
{
    PersistenceRoot = temporaryDirectory,
    Content = new Dictionary<string, ReadOnlyMemory<byte>> { ["textures/atlas.png"] = png },
});
host.Call(engine =>
{
    RenderResource texture = engine.Graphics.OpenResource(new("textures/atlas.png")).Handle;
    using SpriteAtlas atlas = engine.Graphics.CreateSpriteAtlas(new(texture, frames));
    var refusal = Assert.Throws<EngineCallException>(texture.Dispose);
    Assert.Equal("CSHARP_RENDER_RESOURCE_IN_USE", refusal.Diagnostics.Span[0].Code);
});
```

Each `Call` stands for one product callback; make Engine calls inside one.
The renderer work a call produces is dropped. The library ships in the pinned
pair's runtime pack (`lib/librusty_engine_test_host.so`); a project with
`IsTestProject` (set by `Microsoft.NET.Test.Sdk`) that references the product
or `Rusty.Engine` records its path at build time, so `rusty install` is the
only setup. `RustyEngineTestHostLibrary` or `EngineTestHostOptions.LibraryPath`
names another. A library from a different pair refuses with
`CSHARP_TEST_HOST_ABI`. Content bundles, input and product lifecycle are not
part of the test host; drive those through `rusty dev` or the host exercise.

## Tools

A product's own command-line tool (an authoring or simulation CLI, for
example) uses the same in-process service set through `EngineTestHost`. This
is a supported dependency for a shipped tool, not only for tests: the
services, their results and their refusals are the ones a running product
gets, so a tool and the product draw the same `Random` values for the same
seed and scope. Set `RustyEngineToolHost` in the tool's executable project:

```xml
<PropertyGroup>
  <OutputType>Exe</OutputType>
  <RustyEngineToolHost>true</RustyEngineToolHost>
</PropertyGroup>
<ItemGroup>
  <PackageReference Include="Rusty.Engine" Version="[$(RustyEnginePackageVersion)]" />
</ItemGroup>
```

```csharp
using var host = EngineTestHost.Create();
ulong roll = host.Call(engine =>
{
    using Rng dice = engine.Random.CreateScoped(new(seed, "combat"));
    return engine.Random.NextBoundedU32(new(dice, 20)).Value + 1;
});
```

The build copies the pinned pair's `librusty_engine_test_host.so` beside the
tool's build and publish output, so `dotnet run` and a published
(framework-dependent, Linux x64) tool need no `LibraryPath` and no installed
pair on the machine that runs it. The ABI check is the test host's. A tool
gets the services the test host has: no renderer, audio, input or product
lifecycle. It has no build bundles, but it packs and opens
[content containers](csharp-product-project.md#content-containers-installed-beside-the-product)
with `ProductContentBundle.PackContainer(engine.Content, directory, output)` and
`ProductContentBundle.OpenContainer(engine.Content, path)`.

## Update

```bash
rusty update --check
rusty update
```

Only `rusty update` moves the pin. It installs the target pair, rewrites the
pin, and lists the release notes (authored migration notes plus the public API
diff) for every pair since your old pin. Read them, adapt the product, rebuild,
and commit `Directory.Build.props` with the product change.
[Distribution](csharp-distribution.md) explains pairs, the shared cache and
release information.

## Troubleshoot

| Symptom | Meaning and fix |
|---|---|
| `rusty status` says `ready no` | It lists each missing item: pin, installed pair, .NET 10 SDK, `curl`/`tar`. |
| `RUSTY_PAIR_NOT_INSTALLED` | Run `rusty install` in the product repository. Installed pairs work offline. |
| `RUSTY_PIN_MISSING` | Run from the product repository (or pass `--project`); add the pin above. |
| `Rusty Engine pair … is not installed` from a restore | Run `rusty install`. |
| `NU1101`/`NU1603` for Rusty.Engine, or types missing after `rusty update` | The project files let NuGet pick another SDK. `rusty status` names the missing feed declaration or loose reference and prints the fix. |
| `RUSTY_NETWORK` | Only install, update and the first window-mode run need the network; everything else uses the cache. |
| `RUSTY_DEV_RUNTIME_IDENTITY` or an ABI identity mismatch | The package and runtime are from different pairs. Reinstall the pinned pair; never add version negotiation or handwritten interop. |
| `RUSTY_DESKTOP_NOT_PUBLISHED` | The pinned pair has no desktop runtime pack. Move to a newer pair with `rusty update`. |
| hostfxr or CoreCLR fails to load | `rusty dev` sets `DOTNET_ROOT` from `dotnet` on `PATH`; set it yourself when running the host another way. |
| `rusty: not found` under a service manager or Den broker | The bootstrap installs into `~/.local/bin`, which such environments may leave off `PATH`. Launch with `PATH="$HOME/.local/bin:$PATH" exec rusty dev --project …`. |
| A compiler error | `rusty build` and `rusty dev` pass compiler output and exit codes through unchanged. |
| A missing Engine capability | Follow the [missing capability workflow](#missing-capability-workflow). |

For runtime inspection use [playtest inspection](playtest-inspection.md) and
[CoreCLR diagnostics](coreclr-diagnostics.md).

## Capability references

- [Capability map](csharp-capabilities.md): what the C# surface covers.
- [Product project, build and staging](csharp-product-project.md): package,
  product facts, bundled files, UI build, staged bundle, NativeAOT, the host
  exercise contract.
- [Lifecycle and services](csharp-lifecycle.md): the product lifecycle, the
  Engine services and their native-lifetime rules.
- [Managed helpers](csharp-helpers.md): entity and mechanics stores, stats,
  resource tracks.
- [Offline images and GLB export](csharp-offline-images.md).
- [Implicit surfaces](csharp-implicit-surfaces.md): generated implicit-surface
  geometry, plus stats and inventory components, capture/restore, spatial
  debug maps and GLB inspection.
- [World interaction](controller-interaction.md) for containers, doors and
  controller aim assistance; [world streaming contract](world-streaming-contract.md)
  before designing a streaming worker.
- [Product style](csharp-product-style.md): domain layout and thin
  Read → Decide → Apply → Publish coordination. These are conventions, not an
  Engine framework.

## Engine contributors

`rusty dev --engine-source /absolute/rusty-engine` builds against a source
checkout, and `--runtime <runtime-pack>` selects a locally built pack. Both
are explicit; ordinary products never use them or an adjacent checkout.

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
