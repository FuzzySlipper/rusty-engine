# C# SDK guide

A Rusty Engine product is an ordinary C# project that references the
`Rusty.Engine` package and runs on the matching Engine runtime. The `rusty`
command owns the workflow; `rusty --help` and `rusty <command> --help` are
the reference. This page is the short path through it, with links to the
capability references.

## Start

Prerequisites: Linux x64, the .NET 10 SDK, `curl` and `tar` (with `xz` for
window mode). Get `rusty` once (rerun to refresh it):

```bash
curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash
```

Start a new product from [rusty-template](https://github.com/FuzzySlipper/rusty-template),
which is kept on a current pair. An existing product needs one pin plus its feed
declaration in `Directory.Build.props` and exact package references:

```xml
<Project>
  <PropertyGroup>
    <RustyEnginePackageVersion>0.1.0-dev.abc123def456</RustyEnginePackageVersion>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(RUSTY_ENGINE_CACHE)</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == '' and '$(XDG_CACHE_HOME)' != ''">$(XDG_CACHE_HOME)/rusty-engine</RustyEngineCache>
    <RustyEngineCache Condition="'$(RustyEngineCache)' == ''">$(HOME)/.cache/rusty-engine</RustyEngineCache>
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
rusty dev --project src/Game/Game.csproj --port 8787
```

`rusty dev` builds and stages the product, runs it on CoreCLR, and watches the
declared inputs. UI and content-bundle edits reload into the running product;
C#, project and loose-content edits rebuild and replace the runtime. Useful
flags: `--live-debug`, `--debugger` (managed breakpoints, see
[CoreCLR diagnostics](coreclr-diagnostics.md)) and `--headless`.

The runtime renders the world itself with `render-wgpu`. By default it streams
frames to the browser shell page that `rusty dev` serves. The product runs
from load either way. The runtime draws only while a page watches that stream,
but animation and video completions do not wait for one: unwatched, they
advance on Engine time without drawing. `--headless` opens a page in headless
Chromium (`RUSTY_CHROMIUM_PATH` selects it) for an unattended run that wants
frames drawn or the product UI mounted. `RUSTY_RENDER_OUTPUT=window rusty dev …`
presents to a native window instead; the first such run downloads the pair's
desktop runtime pack into the cache (see [desktop shell](desktop-shell.md)).

`rusty build --project …` stages without running; `--aot` also publishes the
NativeAOT product, an explicit fidelity/release check rather than the edit
loop.

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
