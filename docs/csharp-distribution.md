# C# SDK/runtime distribution

An ordinary downstream product consumes one exact release pair: a Linux-x64
archive containing a local `Rusty.Engine` NuGet feed, its matching runtime
pack, and a checksummed pairing manifest. This is a file contract, not a
GitHub-specific setup: obtain the archive and its adjacent `.sha256` file from
the distribution channel available to your environment (see
[Find a published pair](#find-a-published-pair)).

Every pair has a version derived from one Engine commit, for example
`0.1.0-dev.abc123def456`. The archive name, package version, SDK-generated ABI
metadata, and runtime manifest all name that same revision. The host identity
matches the runtime ABI. Do not combine artifacts from different pairs or add
compatibility negotiation.

## Verify and install a pair

Keep the archive and checksum together, then verify before extracting. Run the
checksum command from their containing directory:

```bash
sha256sum -c rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64.tar.gz.sha256
tar -xzf rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64.tar.gz
./rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64/verify-pair.sh \
  --directory ./rusty-engine-csharp-pair-0.1.0-dev.abc123def456-linux-x64
```

The extracted root contains the two consumption inputs plus pair metadata and
its verifier:

```text
sdk-feed/Rusty.Engine.0.1.0-dev.abc123def456.nupkg
runtime-pack/bin/rusty
runtime-pack/bin/rusty-product-host
runtime-pack/runtime-manifest.json
pair-manifest.json
verify-pair.sh
```

Point a product-local `NuGet.Config` at `sdk-feed`, reference the exact
package version, and run the extracted runtime pack:

```bash
/path/to/runtime-pack/bin/rusty dev \
  --project /path/to/Product.Game.csproj \
  --runtime /path/to/runtime-pack
```

Normal downstream consumption needs neither an Engine checkout, Cargo, binding
generation, copied host/browser files, nor a source-development override. The
bundled release pair verifier independently checks all pair payload hashes, SDK
nuspec/props identity, runtime manifest, and the runtime host's `--identity`
output. A `RUSTY_ENGINE_PAIR_*` error means replace
the entire pair with one unmodified matching release artifact.

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
someone changes it.

## What changed in a pair

Each release also carries release information, linked from
`pair-release.json` under `releaseInfo`:

- `release-notes.md`, which is also the release description. It names this pair
  and the previous published pair, gathers the authored migration notes, and
  summarizes the API changes.
- `api-diff.diff`: the public `Rusty.Engine` surface against the previous pair.
  It is absent when the surface did not change or there is no previous pair.
- `api-surface.txt`: this pair's full public surface.

The notes cover one step. To update across several pairs, follow
`releaseInfo.previous` back to your pinned version and read each release. The
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
