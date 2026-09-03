# C# SDK/runtime distribution

An ordinary downstream product consumes one exact release pair: a Linux-x64
archive containing a local `Rusty.Engine` NuGet feed, its matching runtime
pack, and a checksummed pairing manifest. This is a file contract, not a
GitHub-specific setup: obtain the archive and its adjacent `.sha256` file from
the distribution channel available to your environment.

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

## Produce a pair as an Engine contributor

From a clean Engine checkout, build to a new output directory. The script
derives the version from `HEAD`, refuses any tracked or untracked changes, and
never replaces an output path:

```bash
./scripts/build-csharp-release-pair.sh --output /tmp/rusty-engine-release
```

The exact-tag publication workflow accepts only a tag named
`csharp-sdk-v0.1.0-dev.<12-char-HEAD>`. It rebuilds, verifies, clean-consumer
tests, and publishes that archive plus its checksum. There are intentionally
no channels, version aliases, signing layers, compatibility matrices, or
multi-platform releases in this first distribution lane.
