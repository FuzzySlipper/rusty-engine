# asset-pipeline and Doom on published pairs through rusty (#8822)

## Finding

Neither product was actually blocked on an unpublished Engine revision. Each
was on a local build of a `main` commit that later published pairs contain:

- **asset-pipeline** committed `feec788503fe` (published). Only the
  uncommitted working tree of its in-progress task #8694 pinned
  `c3a0f3437708`, a `main` commit that was never published as a pair. The
  newest pair, `35c37d513550`, contains it.
- **Doom** used a local pair: package `playtest-20260928c`, runtime from Engine
  `4cce6e9d569d`. No pair was published at that commit. `e14db30ba217`, two
  commits later, is published; the extra commits only add the capsule
  clearance inspection Doom already uses.

## What changed

Both products now have the #8810 + #8828 shape:

- one pin in `Directory.Build.props`, with the declared pair feed and the
  "not installed: run `rusty install`" target;
- exact `Rusty.Engine` references;
- no `NuGet.Config` feed and no install script;
- launchers on `rusty dev`, using the `~/.local/bin` PATH prefix.

**asset-pipeline `96a972a`** adopts `35c37d513550`.

- One product change: `ViewportScene.Publish` called the removed
  `PublishAttachedSnapshot` (#8736). It now publishes the snapshot, then the
  joint attachments through `PublishChanges` with their child objects as
  upserts.
- `run-workbench.sh` runs `rusty dev`, and the systemd unit's PATH already
  includes `~/.local/bin`.
- `engine_worker.py` (bakes) reads the pinned runtime host from
  `rusty status`.
- `install-engine-pair.sh` and `NuGet.Config` are deleted, and `webapp.md`
  points at `rusty install --archive` for local archives.

**Doom `e21b385`** adopts `e14db30ba217`, with no product code changes.

- `LoadingBay.Game` and both tools examples use the one pin. The examples
  previously pinned `feec788503fe` and restored from asset-pipeline's feed.
- `run-csharp-product.sh` runs `rusty dev`.
- `audit-boundary.mjs` checks for `exec rusty dev` and the exact reference
  instead of the old pack path.
- `NuGet.Config` is deleted, and the playtest and onboarding docs describe
  the pin.

I also tried the newest pair (`35c37d513550`) for Doom: it needs 34 fixes
across the #8741, #8754 and dynamics result-buffer changes. That is a product
adoption for Doom's own lane, so the pin stays on the published pair nearest
its working state.

## Coordination

Both changes were made in separate worktrees from `origin/main` and pushed
there. The live checkouts were not touched:

- **asset-pipeline** still holds #8694's six uncommitted files, including its
  `c3a0f3437708` pin edit.
- **Doom**'s checkout serves a live session on port 4395; pulling would
  restage it mid-session against the new SDK.

#8694 has a Den message explaining that its pin edit is superseded, what to
take when pulling, and that its other edits do not overlap.

## Evidence

**asset-pipeline** on `35c37d513550`:

- Core, Authoring, Cli and App test projects pass (55, 13, 40 and 16).
- App, Bake and Cli build.
- `rusty status` is ready, and the bake worker resolves
  `…/pairs/0.1.0-dev.35c37d513550/runtime-pack/bin/rusty-product-host`.
- `run-workbench.sh`, run with a PATH lacking `~/.local/bin`, served
  `asset-workbench`.
- A real unattended bake was not run. Headless capture on this host is
  #8694's open problem.

**Doom** on `e14db30ba217`:

- `LoadingBay.Game` and both tools examples build, resolving exactly
  `e14db30ba217`.
- The lifecycle exercise passes (recipe weapon/actor animation and Loading
  Bay lifecycle), as does `audit-boundary.mjs`.
- `rusty status` is ready.
- `run-csharp-product.sh`, run with a PATH lacking `~/.local/bin`, served
  `loading-bay.e1m1`.

**This machine**: `~/.local/bin/rusty` was refreshed through the bootstrap to
pair `d189660a00b9`, which carries the #8828 shape check. It reports Dagger
as `project shape  exact pin, pair feed declared`, and `rusty env` is gone.

## Migration

**Doom**: moving to the newest pair means adopting the #8741 guard removal
(`InventoryEdit.Validate`, kinematic `Prepare` to `Step`, trigger reconcile
and restore argument changes), #8754 triggers, the dynamics step-and-read
receipt changes, #8763's `ContentStore` removal and #8736's
`PublishAttachedSnapshot` to `PublishChanges`. `rusty update --check` lists the
notes.
