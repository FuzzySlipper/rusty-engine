# Remaining downstream products on the rusty CLI pin (#8810)

## What changed

Seven products moved to the #8779 shape and kept their current pins:

- one `<RustyEnginePackageVersion>` in `Directory.Build.props`;
- no product installer or update scripts, no `NuGet.Config` feed entry, no
  `.runtime/sdk-feed` copy;
- Den launch commands and documented checks go through `rusty`.

Each change is a local commit on the product's `main`; none is pushed yet.

| Product | Pin | Commit | Removed | Now through `rusty` |
|---|---|---|---|---|
| rusty-crawler | `eedd406900d5` | `a236d0b` | `install-engine-pair.sh`, `update-engine-pin.sh`, `NuGet.Config`, revision property | `verify.sh` (`rusty install` + `export $(rusty env)`), `.den-serve.json`, README |
| rusty-underworld | `360a1ce508a9` | `1e4ccae` | same set | `verify.sh`, `.den-serve.json`, README, playtesting doc. The UI test loads the live-debug module from the cached pinned pack. |
| rusty-dungeon | `360a1ce508a9` | `caa9a26` | `install-engine.sh`, `run-csharp.sh`, `build-csharp.sh`, `NuGet.Config`, revision property | `verify.sh`, `.den-serve.json`, README, AGENTS, architecture |
| rusty-rifles | `a525a33ff441` | `2208a10` | `install-engine.sh`, `dev.sh`, `NuGet.Config`, pin inside the product project | `.den-serve.json`, README, AGENTS |
| rusty-space | `681ec653ab6a` | `c27e82b` | `NuGet.Config`, pin inside the product project | `.den-serve.json`, `.den-playwright.json`, README, AGENTS, docs |
| rusty-d20 | `bcf02594620c` | `bfa0e3c` | `NuGet.Config`, literal versions in two projects | `.den-serve.json`, README, AGENTS, verification docs |
| rusty-roguelike | `bcf02594620c` | `d19c5a7` | `NuGet.Config`, the pin repeated in three projects | `.den-playwright.json`, exercise scripts (runtime pack from `rusty status`), docs |

Ignored `.runtime/` state (persistence, old packs, running live instances) was
left in place. rusty-crawler's pre-existing uncommitted `docs/live-checks.md`
change was not committed.

**Blocked, filed as #8822.** asset-pipeline pins `c3a0f3437708` and Doom pins
`playtest-20260928c`. Neither pair was ever published, and neither has a local
archive, so the CLI cannot install them. Moving them needs a product pin
change.

## Den brokers and PATH

Both Den brokers run commands with `sh -c` under their own `PATH`, which on
this machine does not always include `~/.local/bin` (the crawler loop's
`PATH=/home/agent/.dotnet:/usr/local/bin:/usr/bin:/bin`). Launch commands
therefore use `PATH="$HOME/.local/bin:$PATH" exec rusty dev --project …`.

- Dagger and CraftSurvive were corrected the same way (`a481531`, `b6f1f26`).
- `rusty` was installed for this user through the published bootstrap, into
  `~/.local/bin`.
- The SDK entry page and the distribution guide now name the prefix.

## Evidence

Each product's launch command was run as the broker runs it: `sh -c` with a
`PATH` that lacks `~/.local/bin` (for rusty-rifles the full `PATH` minus
`~/.local/bin`, so its UI build finds pnpm).

| Product | Install | Checks | Broker-style serve |
|---|---|---|---|
| crawler | installed | `rusty build` of `PartyRpg.Host` passes; `verify.sh` stops at a UI test importing `.ts` (pre-existing, rusty-crawler #8823) | served `rusty-crawler-party…` |
| underworld | installed | `verify.sh` green, including the live-debug module test against the cached pack | served `abyssrpg.product` |
| dungeon | installed | `verify.sh` green | served `rusty-dun…` |
| rifles | installed | `rusty build` passes | served `rusty-rifles` |
| space | installed | `rusty build` passes | served `rusty-space` |
| d20 | installed | Core and Product checks pass | served `rusty-d20` |
| roguelike | installed | Product checks pass | see below |

rusty-roguelike's Den project is archived. Two failures there predate this
change; both also occur with its pre-migration commands:

- `rusty dev` stops at `RUSTY_DEV_WATCH_DECLARATION` (declared watch path
  `src/RustyRoguelike.Product/product-ui` does not exist).
- `exercise-product.sh` exits 1 at its persistence check, after finding the
  cached runtime pack and starting the host.

## Migration

For another product:

- Pin once in `Directory.Build.props` and reference
  `Version="$(RustyEnginePackageVersion)"`.
- Delete feed entries and installer scripts.
- Scripts that run plain `dotnet` start with `rusty install` and
  `export $(rusty env)`.
- Scripts that start the host directly read the pack path from
  `rusty status` (`runtime pack` line).
- Den launch commands use `PATH="$HOME/.local/bin:$PATH" exec rusty dev --project …`.
