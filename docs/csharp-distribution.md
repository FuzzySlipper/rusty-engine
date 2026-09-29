# C# SDK/runtime distribution

An ordinary downstream product consumes one exact release pair: a Linux-x64
archive containing a local `Rusty.Engine` NuGet feed, its matching runtime
pack, and a pairing manifest. Every pair has a version derived from one Engine
commit, for example `0.1.0-dev.abc123def456`. The archive name, package
version, SDK-generated ABI metadata, and runtime manifest all name that same
revision. Do not combine artifacts from different pairs or add compatibility
negotiation.

## Use a pair from a product

The `rusty` command owns installing, updating, inspecting and running a
product's pair; `rusty --help` and each command's `--help` are the reference.
Get it once, then refresh it the same way:

```bash
curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash
```

The bootstrap downloads the newest pair, checks its SHA-256, puts that pair's
`rusty` in `~/.local/bin` (`RUSTY_BIN_DIR`), and installs the pair into the
shared cache. Service managers and Den brokers that start commands with their
own `PATH` may not include `~/.local/bin`; launch configurations use
`PATH="$HOME/.local/bin:$PATH" exec rusty dev --project …`.

A product pins exactly one pair with one element in its `Directory.Build.props`.
The same file declares that pair's feed in the shared cache, and every project
references the package exactly (brackets; a bare version is a NuGet minimum and
can resolve a different dev pair from `~/.nuget/packages`):

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

`rusty update` rewrites only the version element. `rusty status` reports a
product whose feed declaration or references are missing and prints the fix.

From the product repository:

```bash
rusty status
rusty install
rusty dev --project src/Game/Game.csproj --port 8787
rusty update --check
rusty update
```

- `rusty install` downloads the pinned pair once into
  `~/.cache/rusty-engine/pairs/<version>` (`RUSTY_ENGINE_CACHE` moves the
  cache), checking the archive's SHA-256 and the manifest's package identity.
  Every product shares the cache, and an installed pair needs no network.
  `--archive <pair.tar.gz>` installs an archive obtained another way; keep its
  `.sha256` beside it.
- Every restore, `rusty build` and plain `dotnet build`/`test`/`run` alike,
  resolves exactly the pin from the cached pair's `sdk-feed`; an uninstalled pin
  fails with "run `rusty install`". `rusty dev` runs the
  pinned pair's own `runtime-pack/bin/rusty`, whose supervisor matches its
  host, and sets `DOTNET_ROOT` from `dotnet` on `PATH` when it is unset.
- `rusty update` is the only thing that moves the pin. It installs the target
  pair (the newest, or `--to <version>`), rewrites the pin, and lists the
  release notes between the old pin and the new one. Commit the changed
  `Directory.Build.props`.
- `rusty status` shows the pin and its file, whether the pair is installed, the
  runtime pack and feed paths, the cached pairs, and missing or mismatched
  prerequisites (the .NET 10 SDK; `curl` and `tar` for installing). It exits 1
  when the product cannot run yet.

Normal downstream consumption needs neither an Engine checkout, Cargo, binding
generation, copied host/browser files, nor a source-development override.
Engine contributors select a runtime explicitly with `rusty dev --runtime
<runtime-pack>` or `--engine-source <checkout>`.

## Verify a pair by hand

The CI consumer check exercises every published archive before publication,
and `rusty dev` compares the host's `--identity` with the runtime manifest at
each launch. To check an extracted pair's full payload yourself, run its
bundled verifier:

```bash
./rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64/verify-pair.sh \
  --directory ./rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64
```

It checks all pair payload hashes, SDK nuspec/props identity, runtime manifest,
and the runtime host's `--identity` output. A `RUSTY_ENGINE_PAIR_*` error means
replace the entire pair with one unmodified matching release artifact.

## Find a published pair

CI publishes every pair as GitHub release `csharp-sdk-v<version>` with the
archive, its checksum, and a small `pair-release.json` that names the version,
source revision, archive URL and SHA-256, and ABI identity. The newest
published pair is GitHub's Latest release, so these two URLs are stable:

```text
https://github.com/FuzzySlipper/rusty-engine/releases/latest/download/pair-release.json
https://github.com/FuzzySlipper/rusty-engine/releases/download/csharp-sdk-v<version>/pair-release.json
```

Latest only makes an update available. A product keeps its explicit pin until
someone runs `rusty update`. `RUSTY_ENGINE_RELEASES` points the CLI and the
bootstrap at another copy of this releases layout.

## What changed in a pair

Each release also carries release information, linked from
`pair-release.json` under `releaseInfo`:

- `release-notes.md`, which is also the release description. It names this pair
  and the previous published pair, gathers the authored migration notes, and
  summarizes the API changes.
- `api-diff.diff`: the public `Rusty.Engine` surface against the previous pair.
  It is absent when the surface did not change or there is no previous pair.
- `api-surface.txt`: this pair's full public surface.

The notes cover one step. `rusty update` follows `releaseInfo.previous` back to
the pinned version and lists each release's notes; pairs published before
release information existed end the chain with a source comparison link. The
API diff shows signatures only. The authored notes carry behaviour, lifecycle
and default changes.

**Writing a note (Engine contributors).** Put a `## Migration` section in the
task's `docs/evidence/<topic>-<task>/README.md`. The next pair includes every
such section that was added or changed since the previous pair. Say what
product code changes, with before/after code where it helps. Name the affected
downstream products when you know them.

To compare any two pairs locally:

```bash
./scripts/build-csharp-release-info.sh --pair new.tar.gz --previous old.tar.gz --output /tmp/release-info
```

## How pairs are published

The `pair` workflow (`.github/workflows/pair.yml`) owns publication. For each
`main` push that changes Rust, C#, the browser shell, fixtures, or the pair
scripts, it builds the pair with `scripts/build-csharp-release-pair.sh`,
exercises that archive with `scripts/test-csharp-release-pair.sh`, builds
release information against the current Latest pair, and publishes the same
archive with `scripts/publish-csharp-release-pair.sh`.
Documentation-only changes do not produce a pair. Run the workflow by hand
(`gh workflow run pair.yml`) to retry a failed publication.

- A revision that already has a published release is skipped; its bytes never
  change.
- Assets are uploaded to a draft, then one edit publishes the release and moves
  Latest. A failed build, consumer check or upload leaves Latest where it was.
- Latest only moves forward: a late publication of an older revision stays
  available by tag without replacing a newer pair.

Contributors do not publish pairs by hand. To inspect a pair locally, build one
from a clean checkout into a new directory and exercise it:

```bash
./scripts/build-csharp-release-pair.sh --output /tmp/rusty-engine-release
./scripts/test-csharp-release-pair.sh /tmp/rusty-engine-release/*.tar.gz
```

Ordinary C# CI still exercises the generated SDK/CoreCLR path. When NativeAOT
fidelity needs verification, run `./scripts/verify-csharp.sh --aot` or dispatch
the C# workflow with its `nativeaot` input, or pass `--aot` to
`test-csharp-release-pair.sh`.
