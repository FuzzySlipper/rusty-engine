# rusty CLI owns the pair pin, install, update and status (#8779)

## What changed

`rusty` gained `status`, `install`, `update`, `build` and `env` next to `dev`
(`rust/crates/rusty-cli/src/pair.rs`, `main.rs`). `rusty --help` and each
command's `--help` carry ordinary product examples.

- **Pin.** One `<RustyEnginePackageVersion>` in the product's
  `Directory.Build.props` (the nearest one at or above the project that
  declares it). MSBuild uses the same property for the package reference.
  Only `rusty update` rewrites it; install and launch never touch it.
- **Cache.** `~/.cache/rusty-engine/pairs/<version>` (`RUSTY_ENGINE_CACHE`,
  else `XDG_CACHE_HOME`). An install downloads the archive and `.sha256`,
  checks the SHA-256 and the manifest's package identity, extracts next to the
  cache and renames into place, so a present directory is a complete pair.
  Later commands trust it with no network and no payload pass.
- **Restore.** `rusty build`/`dev` set `RestoreAdditionalProjectSources` to the
  cached pair's `sdk-feed` (MSBuild reads environment variables as
  properties). Products drop their `NuGet.Config` feed and `.runtime/sdk-feed`
  copies. `rusty env` prints the same variable for plain `dotnet` runs such as
  CI test projects.
- **dev.** Resolves the pin and runs that pair's own `runtime-pack/bin/rusty
  dev --runtime <pack>`, so the supervisor protocol always matches the pair's
  host (older pairs keep working). Sets `DOTNET_ROOT` from `dotnet` on `PATH`
  when unset; the fresh-HOME runs below start the host with `DOTNET_ROOT`
  unset and dotnet in `~/.dotnet`, which previously failed in hostfxr.
- **update.** Newest pair (`releases/latest/download/pair-release.json`) or
  `--to <version>`; `--check` changes nothing. It follows
  `releaseInfo.previous` from the target back to the pin and lists each
  pair's notes and API diff, ending with a source compare link when the chain
  reaches a pair without release information.
- **Bootstrap.** `scripts/install-rusty.sh`, served from `main`: downloads the
  newest pair (or `--version`), checks it, installs its `rusty` into
  `~/.local/bin`, and runs `rusty install --archive` on the same archive.
- **Errors.** Plain messages with a code and the next action; `rusty build`
  passes compiler output and dotnet's exit code through. `--help` now exits 0
  (the two pack tests expected a failing help; updated).

## Removed, kept, and why

- Removed from install: running the pair's `verify-pair.sh`. CI runs the
  consumer check on the exact published bytes, the archive SHA-256 covers
  transport, and `rusty dev` still compares the host `--identity` with the
  runtime manifest each launch. Installing no longer needs `jq` or `unzip`.
- Removed from pins: `RustyEnginePairSourceRevision`. The version names the
  revision and the manifest identity is checked on install.
- Kept: the runtime pack beside the executable as the fallback for an unpinned
  product. Live consumers (rusty-crawler) run a pack's `rusty dev` without
  `--runtime`; the pin takes precedence when present.
- Kept: `--runtime` and `--engine-source` as the explicit contributor
  overrides. No adjacent-checkout discovery.

## Evidence

[`transcript.txt`](transcript.txt) is one run of [`exercise.sh`](exercise.sh)
on 2026-09-29 against the real published pairs, with `HOME` fresh and only
.NET 10, `curl` and `tar` available:

1. The published bootstrap installed `rusty` from pair `0.1.0-dev.12d517f71b68`.
2. A clone of the migrated template at its existing pin `360a1ce508a9`:
   `status` reported not installed (exit 1), `install` downloaded it, `build`
   restored and staged with an empty NuGet folder and no product feed, and
   `dev` delegated to that pair's CLI and served the product.
3. `update --check` listed four pairs' notes plus the compare link;
   `update` installed `12d517f71b68`, rewrote only the pin element
   (`git diff`), and the product built and served on the new pair.
4. Offline (release URL and every proxy unreachable, `obj/` deleted):
   `status` ready, `install` a no-op, `build` and `dev` succeeded;
   `update --check` failed with `RUSTY_NETWORK` naming the URL.
5. `status` with a dotnet 8 shim first on `PATH` and with no dotnet: not ready,
   naming the mismatch; `build` without dotnet: `RUSTY_PREREQUISITE`.

Earlier runs against a local mirror also covered `RUSTY_PAIR_NOT_INSTALLED`
from `dev`, `RUSTY_INSTALL_NOT_PUBLISHED` for a bad pin and `RUSTY_PIN_MISSING`
outside a product. `cargo test -p rusty-cli` (23 + 1) and clippy pass.

## Downstream migration

Each on a local `cli-workflow-8779` branch, keeping its current pin:

| Consumer | Deleted | Now |
|---|---|---|
| rusty-template `d7630ec` | `scripts/install-engine.sh`, `build-csharp.sh`, `run-csharp.sh`, `NuGet.Config`, revision property | README/AGENTS use `rusty` |
| rusty-dagger `3097397` | `scripts/install-engine-pair.sh`, `update-engine-pin.sh`, `NuGet.Config`, revision property | `verify.sh`, sprite workbench, `.den-serve.json`, `playtest.json` use `rusty` |
| rusty-craftsurvive `4fd8a12` | `eng/EnginePair.props` (and five imports), `NuGet.Config`, hand-installed `pair-<rev>` paths | pin in `Directory.Build.props`; Den serve, discovery smoke and CI use `rusty` (`rusty env >> $GITHUB_ENV`) |

Checked: Dagger `verify.sh` installed, restored, built and ran the suites up to
a test that also fails on untouched `origin/main` (rusty-dagger #8811).
CraftSurvive's `RpgCore` check restored through `rusty env` with an empty
package folder, and `rusty dev` served it on its pinned pair. Ignored
`.runtime/` state (persistence, old packs) was left in place.

## Migration

Products move to the CLI workflow as follows:

- Get `rusty` with
  `curl -fsSL https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.sh | bash`.
- Keep one `<RustyEnginePackageVersion>` in `Directory.Build.props` and
  reference `Version="$(RustyEnginePackageVersion)"`. Delete
  `RustyEnginePairSourceRevision`, the `NuGet.Config` entry for a
  product-local feed, and product installer/update scripts.
- Replace `./.runtime/…/runtime-pack/bin/rusty dev --runtime …` with
  `rusty dev --project …`. Replace hand pin bumps with `rusty update`.
- CI or scripts that run plain `dotnet` before any `rusty` restore:
  `export $(rusty env)` (or `rusty env >> "$GITHUB_ENV"`).
- `rusty <command> --help` now exits 0 and prints to stdout.

## Review fix: bootstrap with an older pair

`install-rusty.sh --version <pair before #8779>` used to put that pair's
dev-only `rusty` on PATH and then fail. The bootstrap now runs the candidate
binary from its scratch directory (`install --help`, then `install --archive`)
and replaces the command only after both succeed. Checked with an isolated
`RUSTY_BIN_DIR`: `--version 0.1.0-dev.db2bb445aeaa` exits 1 with a message to
omit `--version`, leaving the existing command byte-identical and the cache
empty. The latest pair (`0.1.0-dev.8f08ab04275f`) installs and replaces it.
